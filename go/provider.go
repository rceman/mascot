package main

// Provider process lifecycle: a coordinator goroutine owns the child, stdin
// writes and session transitions; os/exec drain goroutines own stdout (shared
// framer) and stderr (4096-byte tail + total); exactly one Cmd.Wait goroutine
// per session performs the EOF finalize. Session generation tags all events so
// stale-session output cannot mutate the model or emit records.

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"sort"
	"strconv"
	"sync"
	"time"
)

type eventKind int

const (
	evStarted eventKind = iota
	evChunk
	evTerminal
	evSessionClosed
	evStopped
)

type providerEvent struct {
	kind        eventKind
	generation  uint64
	pid         uint32
	id          uint64
	seq         uint64
	text        string
	receiptQPC  int64
	emitQPC     string
	termKind    string
	lastSeq     int64
	exitCode    int64
	hasExitCode bool
	err         string
	hasErr      bool
}

type workKind int

const (
	workRequest workKind = iota
	workCancel
	workClientResponse
	workSessionFailed
	workSessionEOF
)

type workItem struct {
	kind       workKind
	id         uint64
	prompt     string
	scenario   string
	generation uint64
	requestID  uint64
	err        string
}

type phase int

const (
	phaseAwaitStart phase = iota
	phaseStreaming
	phaseTerminal
)

type requestProtocol struct {
	id                     uint64
	expectedChunks         uint64
	nextSeq                uint64
	phase                  phase
	cancelRequested        bool
	clientRequestSeen      bool
	requiresClientResponse bool
	clientResponseSent     bool
	terminalEmitted        bool
}

func (p *requestProtocol) claimTerminal() (uint64, int64, bool) {
	if p.terminalEmitted {
		return 0, 0, false
	}
	p.terminalEmitted = true
	p.phase = phaseTerminal
	return p.id, int64(p.nextSeq) - 1, true
}

// sharedState mirrors the Rust SharedState. Notify is a capacity-1 channel used
// like a condvar with timeout.
type sharedState struct {
	mu                   sync.Mutex
	notify               chan struct{}
	generation           uint64
	protocol             *requestProtocol
	eof                  bool
	shutdownSent         bool
	shutdownAck          bool
	shutdownRequested    bool
	shutdownDeadline     time.Time
	hasShutdownDeadline  bool
	failed               string
}

func newSharedState() *sharedState {
	return &sharedState{notify: make(chan struct{}, 1)}
}

func (s *sharedState) signal() {
	select {
	case s.notify <- struct{}{}:
	default:
	}
}

// waitFor sleeps until cond reports true or the deadline passes.
func (s *sharedState) waitFor(deadline time.Time, cond func() bool) {
	for {
		s.mu.Lock()
		done := cond()
		s.mu.Unlock()
		if done {
			return
		}
		remaining := time.Until(deadline)
		if remaining <= 0 {
			return
		}
		timer := time.NewTimer(remaining)
		select {
		case <-s.notify:
		case <-timer.C:
		}
		timer.Stop()
	}
}

func (s *sharedState) failedErr() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.failed
}

type stderrTail struct {
	mu    sync.Mutex
	tail  []byte
	total uint64
}

// stderrSink is a Cmd.Stderr io.Writer: retain only the last 4096 bytes plus a
// total byte count. Never fails; runs on an os/exec copy goroutine.
type stderrSink struct {
	tail *stderrTail
}

func (s *stderrSink) Write(b []byte) (int, error) {
	t := s.tail
	t.mu.Lock()
	t.total += uint64(len(b))
	if len(b) >= 4096 {
		t.tail = append(t.tail[:0], b[len(b)-4096:]...)
	} else {
		if over := len(t.tail) + len(b) - 4096; over > 0 {
			t.tail = append(t.tail[:0], t.tail[over:]...)
		}
		t.tail = append(t.tail, b...)
	}
	t.mu.Unlock()
	return len(b), nil
}

type providerConfig struct {
	path            string
	arguments       []string
	cwd             string
	environment     map[string]string
	scenarioChunks  map[string]uint64
	cancelTimeout   time.Duration
	shutdownTimeout time.Duration
}

type provider struct {
	work            *boundedQueue[workItem]
	shared          *sharedState
	stderrTail      *stderrTail
	done            chan struct{}
	shutdownTimeout time.Duration
}

type session struct {
	generation uint64
	cmd        *exec.Cmd
	stdin      io.WriteCloser
	waitDone   chan error
}

func pushEvent(events *boundedQueue[providerEvent], wake func(), ev providerEvent) {
	events.push(ev)
	wake()
}

func pushRecord(records *boundedQueue[string], record string) error {
	if records == nil {
		return nil
	}
	if !records.tryPush(record) {
		return errors.New("control output queue overflow")
	}
	return nil
}

func appendErr(base, extra string) string {
	if base == "" {
		return extra
	}
	return base + "; " + extra
}

func spawnProvider(config providerConfig, uiEvents *boundedQueue[providerEvent], records *boundedQueue[string], wake func()) *provider {
	shared := newSharedState()
	work := newBoundedQueue[workItem](16)
	tail := &stderrTail{}
	p := &provider{
		work:            work,
		shared:          shared,
		stderrTail:      tail,
		done:            make(chan struct{}),
		shutdownTimeout: config.shutdownTimeout,
	}
	c := &coordinator{
		config:   config,
		work:     work,
		shared:   shared,
		uiEvents: uiEvents,
		records:  records,
		tail:     tail,
		wake:     wake,
	}
	go func() {
		c.run()
		close(p.done)
	}()
	return p
}

// sendShutdown requests coordinated shutdown: never pushes to the work queue,
// closes it instead so a blocked reader is released.
func (p *provider) sendShutdown() error {
	s := p.shared
	s.mu.Lock()
	if !s.shutdownRequested {
		s.shutdownRequested = true
		s.shutdownDeadline = time.Now().Add(p.shutdownTimeout)
		s.hasShutdownDeadline = true
	}
	s.mu.Unlock()
	s.signal()
	p.work.close()
	return nil
}

func (p *provider) sendRequest(id uint64, prompt, scenario string) error {
	if !p.work.tryPush(workItem{kind: workRequest, id: id, prompt: prompt, scenario: scenario}) {
		return errors.New("provider command queue unavailable")
	}
	return nil
}

func (p *provider) sendCancel(id uint64) error {
	if !p.work.tryPush(workItem{kind: workCancel, id: id}) {
		return errors.New("provider command queue unavailable")
	}
	return nil
}

func (p *provider) stderrTotal() uint64 {
	t := p.stderrTail
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.total
}

func (p *provider) join(timeout time.Duration) bool {
	select {
	case <-p.done:
		return true
	case <-time.After(timeout):
		return false
	}
}

type coordinator struct {
	config   providerConfig
	work     *boundedQueue[workItem]
	shared   *sharedState
	uiEvents *boundedQueue[providerEvent]
	records  *boundedQueue[string]
	tail     *stderrTail
	wake     func()
}

func (c *coordinator) shutdownRequested() bool {
	s := c.shared
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.shutdownRequested
}

func (c *coordinator) spawnChild() (*session, string) {
	cmd := exec.Command(c.config.path, c.config.arguments...)
	cmd.Dir = c.config.cwd
	keys := make([]string, 0, len(c.config.environment))
	for k := range c.config.environment {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	env := make([]string, 0, len(keys))
	for _, k := range keys {
		env = append(env, k+"="+c.config.environment[k])
	}
	cmd.Env = env
	applyProviderSysProcAttr(cmd)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, "provider stdin pipe: " + err.Error()
	}
	s := c.shared
	s.mu.Lock()
	newGen := s.generation + 1
	s.mu.Unlock()
	sink := &stdoutSink{
		decoder:    newFrameDecoder(),
		generation: newGen,
		shared:     s,
		uiEvents:   c.uiEvents,
		records:    c.records,
		work:       c.work,
		wake:       c.wake,
	}
	cmd.Stdout = sink
	cmd.Stderr = &stderrSink{tail: c.tail}
	if err := cmd.Start(); err != nil {
		return nil, "provider spawn: " + err.Error()
	}
	pid := cmd.Process.Pid
	s.mu.Lock()
	s.generation = newGen
	s.eof = false
	s.failed = ""
	s.protocol = nil
	s.shutdownSent = false
	s.shutdownAck = false
	s.mu.Unlock()
	sess := &session{
		generation: newGen,
		cmd:        cmd,
		stdin:      stdin,
		waitDone:   make(chan error, 1),
	}
	go func() {
		// Exactly one Cmd.Wait goroutine per session; Wait also joins the
		// internal stdout/stderr copy goroutines, so this finalize runs only
		// after every Write (and thus every queued frame) has been consumed.
		err := cmd.Wait()
		sink.finish("")
		sess.waitDone <- err
	}()
	pushEvent(c.uiEvents, c.wake, providerEvent{kind: evStarted, generation: newGen, pid: uint32(pid)})
	return sess, ""
}

func writeFrame(stdin io.Writer, fields map[string]any) error {
	frame, err := json.Marshal(fields)
	if err != nil {
		return err
	}
	frame = append(frame, '\n')
	n, err := stdin.Write(frame)
	if err == nil && n != len(frame) {
		err = io.ErrShortWrite
	}
	return err
}

// teardown mirrors Rust: on an error teardown allow ~100ms grace before
// Process.Kill so the overall reap stays well inside the shared deadline.
func (c *coordinator) teardown(sess **session, errStr string, deadline time.Time) string {
	active := *sess
	if active == nil {
		return errStr
	}
	*sess = nil
	grace := deadline
	if errStr != "" {
		grace = time.Now().Add(100 * time.Millisecond)
		if grace.After(deadline) {
			grace = deadline
		}
	}
	reaped := false
	wait := time.Until(grace)
	if wait < 0 {
		wait = 0
	}
	select {
	case <-active.waitDone:
		reaped = true
	case <-time.After(wait):
	}
	if !reaped {
		if err := active.cmd.Process.Kill(); err != nil {
			errStr = appendErr(errStr, "provider kill failed")
		}
		select {
		case <-active.waitDone:
			reaped = true
		case <-time.After(time.Until(deadline)):
			errStr = appendErr(errStr, "provider did not exit before teardown deadline")
		}
	}
	var exitCode int64
	hasExitCode := false
	if reaped {
		if ps := active.cmd.ProcessState; ps != nil {
			exitCode = int64(ps.ExitCode())
			hasExitCode = true
		}
	}
	active.stdin.Close()
	pushEvent(c.uiEvents, c.wake, providerEvent{
		kind:        evSessionClosed,
		generation:  active.generation,
		exitCode:    exitCode,
		hasExitCode: hasExitCode,
		err:         errStr,
		hasErr:      errStr != "",
	})
	return errStr
}

// serviceClientResponse answers the frozen benchmark.confirm client request at
// most once per request, bounded by the given deadline.
func (c *coordinator) serviceClientResponse(active *session, deadline time.Time) string {
	s := c.shared
	for {
		s.mu.Lock()
		p := s.protocol
		if p == nil || !p.requiresClientResponse || p.clientResponseSent || s.eof || s.failed != "" {
			s.mu.Unlock()
			return ""
		}
		if p.clientRequestSeen {
			s.mu.Unlock()
			break
		}
		remaining := time.Until(deadline)
		s.mu.Unlock()
		if remaining <= 0 {
			return "client_request response deadline exceeded"
		}
		timer := time.NewTimer(remaining)
		select {
		case <-s.notify:
		case <-timer.C:
		}
		timer.Stop()
	}
	if err := writeFrame(active.stdin, map[string]any{
		"type":       "client_response",
		"request_id": 1,
		"result":     map[string]any{"accepted": true},
	}); err != nil {
		return "client response write failed"
	}
	s.mu.Lock()
	if p := s.protocol; p != nil {
		p.clientResponseSent = true
	}
	s.mu.Unlock()
	return ""
}

func (c *coordinator) gracefulShutdown(sess **session, timeout time.Duration) string {
	s := c.shared
	s.mu.Lock()
	deadline := s.shutdownDeadline
	if !s.hasShutdownDeadline {
		deadline = time.Now().Add(timeout)
		s.shutdownDeadline = deadline
		s.hasShutdownDeadline = true
	}
	s.mu.Unlock()

	var errStr string
	if active := *sess; active != nil {
		ackDeadline := deadline.Add(-100 * time.Millisecond)
		if ackDeadline.Before(time.Now()) {
			ackDeadline = time.Now()
		}
		_ = c.serviceClientResponse(active, ackDeadline)
		s.mu.Lock()
		s.shutdownSent = true
		s.mu.Unlock()
		if err := writeFrame(active.stdin, map[string]any{"type": "shutdown"}); err != nil {
			errStr = "provider shutdown write failed"
		}
		s.waitFor(ackDeadline, func() bool {
			return s.eof || s.shutdownAck
		})
		s.mu.Lock()
		acknowledged := s.shutdownAck
		s.mu.Unlock()
		if !acknowledged && errStr == "" {
			errStr = "provider shutdown acknowledgement timeout"
		}
	}
	return c.teardown(sess, errStr, deadline)
}

func (c *coordinator) run() {
	var sess *session
	for {
		if c.shutdownRequested() {
			break
		}
		item, ok := c.work.waitPop()
		if !ok {
			break
		}
		switch item.kind {
		case workRequest:
			expected, ok := c.config.scenarioChunks[item.scenario]
			if !ok {
				expected = 100
			}
			if sess == nil || c.shared.failedErr() != "" {
				c.teardown(&sess, "", time.Now().Add(c.config.shutdownTimeout))
				s, errStr := c.spawnChild()
				if errStr != "" {
					c.shared.mu.Lock()
					gen := c.shared.generation
					c.shared.mu.Unlock()
					pushEvent(c.uiEvents, c.wake, providerEvent{
						kind: evTerminal, generation: gen, id: item.id,
						termKind: "failed", lastSeq: -1,
					})
					pushEvent(c.uiEvents, c.wake, providerEvent{
						kind: evSessionClosed, generation: gen, err: errStr, hasErr: true,
					})
					continue
				}
				sess = s
			}
			c.shared.mu.Lock()
			c.shared.protocol = &requestProtocol{
				id:                     item.id,
				expectedChunks:         expected,
				phase:                  phaseAwaitStart,
				requiresClientResponse: item.scenario == "client_request",
			}
			c.shared.mu.Unlock()
			if sess != nil {
				if err := writeFrame(sess.stdin, map[string]any{
					"type":     "request",
					"id":       item.id,
					"prompt":   item.prompt,
					"scenario": item.scenario,
				}); err != nil {
					c.shared.mu.Lock()
					c.shared.failed = "provider stdin write failed"
					var claimID uint64
					var claimSeq int64
					claimed := false
					if p := c.shared.protocol; p != nil {
						claimID, claimSeq, claimed = p.claimTerminal()
					}
					gen := uint64(0)
					if sess != nil {
						gen = sess.generation
					}
					c.shared.mu.Unlock()
					c.shared.signal()
					if claimed {
						pushEvent(c.uiEvents, c.wake, providerEvent{
							kind: evTerminal, generation: gen, id: claimID,
							termKind: "failed", lastSeq: claimSeq,
						})
					}
					c.teardown(&sess, "stdin write failed", time.Now().Add(c.config.shutdownTimeout))
				}
			}
		case workCancel:
			active := sess
			if active == nil {
				break
			}
			c.shared.mu.Lock()
			if p := c.shared.protocol; p != nil && p.id == item.id {
				p.cancelRequested = true
			}
			c.shared.mu.Unlock()
			cancelDeadline := time.Now().Add(c.config.cancelTimeout)
			if errStr := c.serviceClientResponse(active, cancelDeadline); errStr != "" {
				c.shared.mu.Lock()
				c.shared.failed = errStr
				c.shared.mu.Unlock()
				c.shared.signal()
				c.teardown(&sess, errStr, time.Now().Add(c.config.shutdownTimeout))
				continue
			}
			_ = writeFrame(active.stdin, map[string]any{"type": "cancel", "id": item.id})
			c.shared.waitFor(cancelDeadline, func() bool {
				s := c.shared
				terminal := s.protocol != nil && s.protocol.phase == phaseTerminal
				return terminal || s.failed != "" || s.eof || s.shutdownRequested
			})
			c.shared.mu.Lock()
			terminalSeen := c.shared.protocol != nil && c.shared.protocol.phase == phaseTerminal
			aborted := c.shared.failed != "" || c.shared.eof || c.shared.shutdownRequested
			var claimID uint64
			var claimSeq int64
			claimed := false
			if !terminalSeen && !aborted {
				c.shared.failed = "cancel deadline exceeded"
				if p := c.shared.protocol; p != nil {
					claimID, claimSeq, claimed = p.claimTerminal()
				}
			}
			c.shared.mu.Unlock()
			if claimed {
				c.shared.signal()
				gen := uint64(0)
				if sess != nil {
					gen = sess.generation
				}
				pushEvent(c.uiEvents, c.wake, providerEvent{
					kind: evTerminal, generation: gen, id: claimID,
					termKind: "failed", lastSeq: claimSeq,
				})
				c.teardown(&sess, "cancel deadline exceeded", time.Now().Add(c.config.shutdownTimeout))
			}
		case workClientResponse:
			if active := sess; active != nil {
				c.shared.mu.Lock()
				current := c.shared.generation == item.generation &&
					c.shared.protocol != nil && c.shared.protocol.id == item.id &&
					item.requestID == 1
				c.shared.mu.Unlock()
				if current {
					deadline := time.Now().Add(c.config.cancelTimeout)
					if errStr := c.serviceClientResponse(active, deadline); errStr != "" {
						c.shared.mu.Lock()
						c.shared.failed = errStr
						c.shared.mu.Unlock()
						c.shared.signal()
					}
				}
			}
		case workSessionFailed:
			if sess != nil && sess.generation == item.generation {
				c.teardown(&sess, item.err, time.Now().Add(c.config.shutdownTimeout))
			}
		case workSessionEOF:
			if sess != nil && sess.generation == item.generation {
				c.teardown(&sess, "", time.Now().Add(c.config.shutdownTimeout))
			}
		}
		if c.shutdownRequested() {
			break
		}
	}
	errStr := c.gracefulShutdown(&sess, c.config.shutdownTimeout)
	pushEvent(c.uiEvents, c.wake, providerEvent{
		kind: evStopped, err: errStr, hasErr: errStr != "",
	})
}

// stdoutSink is a Cmd.Stdout io.Writer driven by an os/exec copy goroutine.
// It feeds the shared NDJSON decoder; a fatal decode/protocol error is
// finalized immediately so the coordinator can tear the session down without
// waiting for process exit.
type stdoutSink struct {
	decoder    *frameDecoder
	generation uint64
	shared     *sharedState
	uiEvents   *boundedQueue[providerEvent]
	records    *boundedQueue[string]
	work       *boundedQueue[workItem]
	wake       func()
	ended      bool
}

func (s *stdoutSink) Write(b []byte) (int, error) {
	if s.ended {
		return len(b), nil
	}
	err := s.decoder.feed(b, qpc, func(frame []byte, receipt int64) error {
		return handleFrame(frame, receipt, s.generation, s.shared, s.uiEvents, s.records, s.work, s.wake)
	})
	fatal := ""
	if err != nil {
		fatal = err.Error()
	} else if failed := s.shared.failedErr(); failed != "" {
		fatal = failed
	}
	if fatal == "" {
		return len(b), nil
	}
	s.finish(fatal)
	return 0, errors.New(fatal)
}

// finish runs the equivalent of the Rust reader's post-loop block: mark EOF,
// validate the stream tail, claim the failed terminal and enqueue the session
// work item. Idempotent; invoked either from Write on a fatal error or from the
// session's single Wait goroutine after a clean EOF.
func (s *stdoutSink) finish(fatal string) {
	shared := s.shared
	shared.mu.Lock()
	if s.ended {
		shared.mu.Unlock()
		return
	}
	s.ended = true
	shared.eof = true
	if fatal == "" {
		if err := s.decoder.finish(); err != nil {
			fatal = err.Error()
		}
	}
	if fatal == "" {
		if p := shared.protocol; p != nil && p.phase != phaseTerminal &&
			!shared.shutdownRequested && !shared.shutdownSent {
			fatal = "provider stdout ended mid-request"
		}
	}
	if fatal != "" && shared.failed == "" {
		shared.failed = fatal
	}
	var claimID uint64
	var claimSeq int64
	claimed := false
	if fatal != "" && shared.protocol != nil {
		claimID, claimSeq, claimed = shared.protocol.claimTerminal()
	}
	shared.mu.Unlock()
	shared.signal()
	if fatal != "" {
		if claimed {
			pushEvent(s.uiEvents, s.wake, providerEvent{
				kind: evTerminal, generation: s.generation, id: claimID,
				termKind: "failed", lastSeq: claimSeq,
			})
		}
		if !s.work.tryPush(workItem{kind: workSessionFailed, generation: s.generation, err: fatal}) {
			shared.mu.Lock()
			shutting := shared.shutdownRequested
			shared.mu.Unlock()
			if !shutting {
				_ = pushRecord(s.records, `{"token":null,"ok":false,"error":"provider work queue unavailable"}`)
			}
		}
	} else {
		s.work.tryPush(workItem{kind: workSessionEOF, generation: s.generation})
	}
}

func jsonStringField(v map[string]json.RawMessage, key string) (string, bool) {
	raw, ok := v[key]
	if !ok {
		return "", false
	}
	var s string
	if err := json.Unmarshal(raw, &s); err != nil {
		return "", false
	}
	return s, true
}

func jsonUintField(v map[string]json.RawMessage, key string) (uint64, bool) {
	raw, ok := v[key]
	if !ok {
		return 0, false
	}
	var n uint64
	if err := json.Unmarshal(raw, &n); err != nil {
		return 0, false
	}
	return n, true
}

func jsonIntField(v map[string]json.RawMessage, key string) (int64, bool) {
	raw, ok := v[key]
	if !ok {
		return 0, false
	}
	var n int64
	if err := json.Unmarshal(raw, &n); err != nil {
		return 0, false
	}
	return n, true
}

// handleFrame validates one provider frame and emits the resulting records and
// UI events. Mirrors rust/src/provider.rs handle_frame exactly.
func handleFrame(frame []byte, receiptQPC int64, generation uint64, shared *sharedState,
	uiEvents *boundedQueue[providerEvent], records *boundedQueue[string],
	work *boundedQueue[workItem], wake func()) error {

	var value map[string]json.RawMessage
	if err := json.Unmarshal(frame, &value); err != nil {
		return fmt.Errorf("invalid frame JSON: %s", err)
	}
	kind, ok := jsonStringField(value, "type")
	if !ok {
		return errors.New("frame missing type")
	}
	frameID, _ := jsonUintField(value, "id")

	type deliverKind int
	const (
		deliverNone deliverKind = iota
		deliverChunk
		deliverTerminal
	)
	deliver := deliverNone
	var chunkSeq uint64
	var chunkText, chunkEmit string
	var termKind string
	var termLastSeq int64
	var frameRecord string

	shared.mu.Lock()
	if shared.generation != generation {
		shared.mu.Unlock()
		return errors.New("stale session frame")
	}
	switch kind {
	case "start":
		p := shared.protocol
		if p == nil {
			shared.mu.Unlock()
			return errors.New("start without active request")
		}
		if p.id != frameID {
			shared.mu.Unlock()
			return errors.New("start id mismatch")
		}
		if p.phase != phaseAwaitStart {
			shared.mu.Unlock()
			return errors.New("duplicate start")
		}
		chunks, ok := jsonUintField(value, "chunks")
		if !ok {
			shared.mu.Unlock()
			return errors.New("start missing chunks")
		}
		if chunks != p.expectedChunks {
			shared.mu.Unlock()
			return errors.New("start chunk count mismatch")
		}
		frequency, ok := jsonIntField(value, "qpc_frequency")
		if !ok {
			shared.mu.Unlock()
			return errors.New("start missing qpc_frequency")
		}
		if frequency != qpcFrequency() {
			shared.mu.Unlock()
			return errors.New("qpc frequency mismatch")
		}
		p.phase = phaseStreaming
	case "chunk":
		p := shared.protocol
		if p == nil {
			shared.mu.Unlock()
			return errors.New("chunk without request")
		}
		if p.id != frameID {
			shared.mu.Unlock()
			return errors.New("chunk id mismatch")
		}
		if p.phase != phaseStreaming {
			shared.mu.Unlock()
			return errors.New("chunk outside streaming")
		}
		seq, ok := jsonUintField(value, "seq")
		if !ok {
			shared.mu.Unlock()
			return errors.New("chunk missing seq")
		}
		if seq != p.nextSeq || seq >= p.expectedChunks {
			shared.mu.Unlock()
			return errors.New("chunk seq out of order")
		}
		text, ok := jsonStringField(value, "text")
		if !ok {
			shared.mu.Unlock()
			return errors.New("chunk missing text")
		}
		emitQPC, ok := jsonStringField(value, "emit_qpc")
		if !ok {
			shared.mu.Unlock()
			return errors.New("chunk missing emit_qpc")
		}
		if len(emitQPC) != 20 {
			shared.mu.Unlock()
			return errors.New("invalid emit_qpc")
		}
		valid := true
		for i := 0; i < len(emitQPC); i++ {
			if emitQPC[i] < '0' || emitQPC[i] > '9' {
				valid = false
				break
			}
		}
		if valid {
			// i64 parse with overflow rejection, matching the reference.
			parsed, perr := strconv.ParseInt(emitQPC, 10, 64)
			valid = perr == nil && parsed > 0
		}
		if !valid {
			shared.mu.Unlock()
			return errors.New("invalid emit_qpc")
		}
		p.nextSeq++
		record, _ := json.Marshal(map[string]any{
			"event":         "frame_received",
			"request_id":    frameID,
			"seq":           seq,
			"receipt_qpc":   json.Number(fmt.Sprintf("%d", receiptQPC)).String(),
			"emit_qpc":      emitQPC,
			"qpc_frequency": qpcFrequency(),
		})
		frameRecord = string(record)
		deliver = deliverChunk
		chunkSeq = seq
		chunkText = text
		chunkEmit = emitQPC
	case "client_request":
		p := shared.protocol
		if p == nil {
			shared.mu.Unlock()
			return errors.New("client_request without request")
		}
		if p.id != frameID {
			shared.mu.Unlock()
			return errors.New("client_request id mismatch")
		}
		if p.phase != phaseStreaming {
			shared.mu.Unlock()
			return errors.New("client_request outside streaming")
		}
		requestID, ok := jsonUintField(value, "request_id")
		if !ok {
			shared.mu.Unlock()
			return errors.New("client_request missing request_id")
		}
		method, ok := jsonStringField(value, "method")
		if !ok {
			shared.mu.Unlock()
			return errors.New("client_request missing method")
		}
		paramsOK := false
		if raw, exists := value["params"]; exists {
			var params map[string]json.RawMessage
			if err := json.Unmarshal(raw, &params); err == nil {
				v, _ := jsonStringField(params, "value")
				paramsOK = v == "ok"
			}
		}
		if requestID != 1 || method != "benchmark.confirm" || !paramsOK {
			shared.mu.Unlock()
			return errors.New("unexpected client_request")
		}
		if p.clientRequestSeen {
			shared.mu.Unlock()
			return errors.New("duplicate client_request")
		}
		p.clientRequestSeen = true
		activeID := p.id
		shared.mu.Unlock()
		shared.signal()
		if !work.tryPush(workItem{kind: workClientResponse, generation: generation, id: activeID, requestID: requestID}) {
			shared.mu.Lock()
			shutting := shared.shutdownRequested
			shared.mu.Unlock()
			if shutting {
				return nil
			}
			_ = pushRecord(records, `{"token":null,"ok":false,"error":"provider work queue overflow"}`)
			return errors.New("provider work queue overflow")
		}
		return nil
	case "complete":
		p := shared.protocol
		if p == nil {
			shared.mu.Unlock()
			return errors.New("complete without request")
		}
		if p.id != frameID {
			shared.mu.Unlock()
			return errors.New("complete id mismatch")
		}
		if p.phase != phaseStreaming {
			shared.mu.Unlock()
			return errors.New("complete outside streaming")
		}
		if p.nextSeq != p.expectedChunks {
			shared.mu.Unlock()
			return errors.New("premature complete")
		}
		id, lastSeq, ok := p.claimTerminal()
		if !ok {
			shared.mu.Unlock()
			return errors.New("duplicate terminal")
		}
		deliver = deliverTerminal
		termKind = "complete"
		termLastSeq = lastSeq
		_ = id
	case "cancelled":
		shutdownSent := shared.shutdownSent || shared.shutdownRequested
		p := shared.protocol
		if p == nil {
			shared.mu.Unlock()
			return errors.New("cancelled without request")
		}
		if p.id != frameID {
			shared.mu.Unlock()
			return errors.New("cancelled id mismatch")
		}
		if p.phase == phaseTerminal || p.terminalEmitted {
			shared.mu.Unlock()
			return errors.New("duplicate terminal")
		}
		if !p.cancelRequested && !shutdownSent {
			shared.mu.Unlock()
			return errors.New("cancelled without request")
		}
		lastSeq, ok := jsonIntField(value, "last_seq")
		if !ok {
			shared.mu.Unlock()
			return errors.New("cancelled missing last_seq")
		}
		if lastSeq != int64(p.nextSeq)-1 {
			shared.mu.Unlock()
			return errors.New("cancelled last_seq mismatch")
		}
		_, seq, ok := p.claimTerminal()
		if !ok {
			shared.mu.Unlock()
			return errors.New("duplicate terminal")
		}
		deliver = deliverTerminal
		termKind = "cancelled"
		termLastSeq = seq
	case "shutdown_ack":
		if !shared.shutdownSent {
			shared.mu.Unlock()
			return errors.New("unsolicited shutdown_ack")
		}
		if shared.shutdownAck {
			shared.mu.Unlock()
			return errors.New("duplicate shutdown_ack")
		}
		shared.shutdownAck = true
	default:
		shared.mu.Unlock()
		return fmt.Errorf("unexpected frame type '%s'", kind)
	}
	shared.mu.Unlock()
	shared.signal()

	if frameRecord != "" {
		if err := pushRecord(records, frameRecord); err != nil {
			return err
		}
	}
	switch deliver {
	case deliverChunk:
		pushEvent(uiEvents, wake, providerEvent{
			kind: evChunk, generation: generation, id: frameID, seq: chunkSeq,
			text: chunkText, receiptQPC: receiptQPC, emitQPC: chunkEmit,
		})
	case deliverTerminal:
		pushEvent(uiEvents, wake, providerEvent{
			kind: evTerminal, generation: generation, id: frameID,
			termKind: termKind, lastSeq: termLastSeq,
		})
	}
	return nil
}
