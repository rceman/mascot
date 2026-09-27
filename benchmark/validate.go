package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"
	"unicode/utf8"
)

type decoder struct {
	buffer [frameLimit]byte
	used   int
	peak   int
}

func (value *decoder) feed(data []byte) ([][]byte, error) {
	var frames [][]byte
	for _, item := range data {
		if value.used == len(value.buffer) {
			return frames, errors.New("frame limit exceeded")
		}
		value.buffer[value.used] = item
		value.used++
		if value.used > value.peak {
			value.peak = value.used
		}
		if item == '\n' {
			if !utf8.Valid(value.buffer[:value.used]) {
				return frames, errors.New("invalid UTF-8")
			}
			frames = append(frames, append([]byte{}, value.buffer[:value.used]...))
			value.used = 0
		}
	}
	return frames, nil
}

type frameSink struct {
	buffer []byte
	frames chan []byte
	abort  <-chan struct{}
}

func (sink *frameSink) Write(data []byte) (int, error) {
	for index, item := range data {
		if len(sink.buffer) == frameLimit+1 {
			return index, errors.New("fixture output exceeds oversized test bound")
		}
		sink.buffer = append(sink.buffer, item)
		if item == '\n' {
			frame := append([]byte{}, sink.buffer...)
			select {
			case sink.frames <- frame:
			case <-sink.abort:
			}
			sink.buffer = sink.buffer[:0]
		}
	}
	return len(data), nil
}

type diagnosticSink struct {
	data []byte
}

func (sink *diagnosticSink) Write(data []byte) (int, error) {
	if len(sink.data)+len(data) > 1024*1024 {
		return 0, errors.New("stderr fixture exceeded bound")
	}
	sink.data = append(sink.data, data...)
	return len(data), nil
}

type session struct {
	cmd    *exec.Cmd
	stdin  io.WriteCloser
	frames chan []byte
	done   chan error
	abort  chan struct{}
	once   sync.Once
	stderr *diagnosticSink
	waited bool
}

func launchProvider(root string) (*session, error) {
	executable, err := os.Executable()
	if err != nil {
		return nil, err
	}
	value := &session{frames: make(chan []byte, 8), done: make(chan error, 1), abort: make(chan struct{}), stderr: &diagnosticSink{}}
	value.cmd = exec.Command(executable, "provider", root)
	value.cmd.Dir = root
	value.cmd.Env = environmentList()
	value.cmd.Stdout = &frameSink{frames: value.frames, abort: value.abort}
	value.cmd.Stderr = value.stderr
	value.stdin, err = value.cmd.StdinPipe()
	if err != nil {
		return nil, err
	}
	if err := value.cmd.Start(); err != nil {
		value.stdin.Close()
		return nil, err
	}
	go func() {
		err := value.cmd.Wait()
		close(value.frames)
		value.done <- err
	}()
	return value, nil
}

func (value *session) send(message map[string]any) error {
	_, err := value.stdin.Write(wire(message))
	return err
}

func (value *session) next() ([]byte, map[string]any, error) {
	select {
	case data, ok := <-value.frames:
		if !ok {
			return nil, nil, io.EOF
		}
		var message map[string]any
		parser := json.NewDecoder(bytes.NewReader(data))
		parser.UseNumber()
		if err := parser.Decode(&message); err != nil {
			return data, nil, err
		}
		return data, message, nil
	case <-time.After(3 * time.Second):
		return nil, nil, errors.New("provider frame timeout")
	}
}

func (value *session) wait() error {
	if value.waited {
		return nil
	}
	select {
	case err := <-value.done:
		value.waited = true
		return err
	case <-time.After(2 * time.Second):
		return errors.New("provider exit timeout")
	}
}

func (value *session) cleanup() {
	value.once.Do(func() { close(value.abort) })
	if !value.waited {
		value.cmd.Process.Kill()
		value.wait()
	}
	value.stdin.Close()
}

func (value *session) shutdown() error {
	if err := value.send(map[string]any{"type": "shutdown"}); err != nil {
		return err
	}
	_, message, err := value.next()
	if err != nil {
		return err
	}
	if message["type"] != "shutdown_ack" {
		return errors.New("shutdown acknowledgement missing")
	}
	return value.wait()
}

func checkStream(value *session, id int64, scenario string, payload []string) (int, error) {
	if err := value.send(map[string]any{"type": "request", "id": id, "prompt": "fixture validation", "scenario": scenario}); err != nil {
		return 0, err
	}
	_, start, err := value.next()
	if err != nil {
		return 0, err
	}
	want := 100
	if scenario == "backpressure" {
		want = 256
	}
	if scenario == "maximum" || scenario == "oversized" {
		want = 1
	}
	chunks, err := decimal(start["chunks"])
	startID, idErr := decimal(start["id"])
	clockFrequency, clockErr := decimal(start["qpc_frequency"])
	if err != nil || idErr != nil || clockErr != nil || start["type"] != "start" || chunks != int64(want) || startID != id || clockFrequency != frequency() {
		return 0, errors.New("incorrect start frame")
	}
	if scenario == "client_request" {
		_, request, err := value.next()
		if err != nil {
			return 0, err
		}
		requestID, _ := decimal(request["request_id"])
		params, _ := request["params"].(map[string]any)
		if request["type"] != "client_request" || requestID != 1 || request["method"] != "benchmark.confirm" || params["value"] != "ok" {
			return 0, errors.New("incorrect client request")
		}
		if err := value.send(map[string]any{"type": "client_response", "request_id": 1, "result": map[string]any{"accepted": true}}); err != nil {
			return 0, err
		}
	}
	count := 0
	lastTimestamp := int64(0)
	var response strings.Builder
	for {
		data, message, err := value.next()
		if errors.Is(err, io.EOF) && scenario == "unexpected_exit" && count == 7 {
			err = value.wait()
			var exit *exec.ExitError
			if !errors.As(err, &exit) || exit.ExitCode() != 23 {
				return count, errors.New("unexpected-exit status is not23")
			}
			return count, nil
		}
		if err != nil {
			return count, err
		}
		messageID, err := decimal(message["id"])
		if err != nil || messageID != id {
			return count, errors.New("response request ID mismatch")
		}
		switch message["type"] {
		case "chunk":
			sequence, err := decimal(message["seq"])
			if err != nil || sequence != int64(count) {
				return count, errors.New("chunk order mismatch")
			}
			timestamp, err := decimal(message["emit_qpc"])
			if err != nil || timestamp <= 0 || timestamp < lastTimestamp || timestamp > qpc() {
				return count, errors.New("invalid QPC emission timestamp")
			}
			lastTimestamp = timestamp
			text, ok := message["text"].(string)
			if !ok {
				return count, errors.New("text missing")
			}
			if scenario == "oversized" {
				var parser decoder
				if len(data) != frameLimit+1 {
					return count, errors.New("wrong oversized length")
				}
				if _, err := parser.feed(data); err == nil {
					return count, errors.New("reference decoder accepted oversized frame")
				}
				began := time.Now()
				value.cmd.Process.Kill()
				if err := value.wait(); err == nil || time.Since(began) >= 2*time.Second {
					return count, errors.New("oversized invalidation did not reap in bound")
				}
				return 1, nil
			}
			if scenario == "maximum" {
				if len(data) != frameLimit || text != strings.Repeat("x", frameLimit-len(chunk(id, 0, ""))) {
					return count, errors.New("wrong maximum frame")
				}
			} else if text != payload[count%len(payload)] {
				return count, errors.New("payload differs from fixture")
			}
			var parser decoder
			if _, err := parser.feed(data); err != nil {
				return count, err
			}
			response.WriteString(text)
			count++
			if scenario == "cancel" && count == 50 {
				if err := value.send(map[string]any{"type": "cancel", "id": id}); err != nil {
					return count, err
				}
			}
		case "cancelled":
			last, err := decimal(message["last_seq"])
			if scenario != "cancel" || count != 50 || last != 49 || err != nil {
				return count, errors.New("incorrect cancellation barrier terminal")
			}
			return count, nil
		case "complete":
			if scenario == "cancel" || count != want {
				return count, errors.New("incorrect complete terminal")
			}
			if want == 100 && response.String() != strings.Join(payload, "") {
				return count, errors.New("response reconstruction mismatch")
			}
			return count, nil
		default:
			return count, errors.New("unexpected frame")
		}
	}
}

func validate(root string) error {
	data, err := os.ReadFile(filepath.Join(root, "manifest", "fixture.json"))
	if err != nil {
		return err
	}
	var manifest struct {
		Version  string            `json:"version"`
		Files    map[string]string `json:"files_sha256"`
		Provider struct {
			SHA     string            `json:"sha256"`
			Env     map[string]string `json:"environment"`
			Inherit bool              `json:"inherit_environment"`
		} `json:"provider"`
	}
	if err := json.Unmarshal(data, &manifest); err != nil {
		return err
	}
	if manifest.Version != fixtureVersion || len(manifest.Files) != 8 || manifest.Provider.Inherit {
		return errors.New("manifest identity/content invalid")
	}
	for path, expected := range manifest.Files {
		actual, err := hashFile(filepath.Join(root, filepath.FromSlash(path)))
		if err != nil || actual != expected {
			return fmt.Errorf("fixture hash mismatch: %s", path)
		}
	}
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	binaryHash, err := hashFile(executable)
	if err != nil || binaryHash != manifest.Provider.SHA {
		return errors.New("provider executable identity mismatch")
	}
	if len(manifest.Provider.Env) != len(providerEnvironment()) {
		return errors.New("unexpected provider environment keys")
	}
	for key, value := range providerEnvironment() {
		if manifest.Provider.Env[key] != value {
			return fmt.Errorf("provider environment differs: %s", key)
		}
	}
	environmentProbe := exec.Command(executable, "environment", root)
	environmentProbe.Dir = root
	environmentProbe.Env = environmentList()
	environmentBytes, err := environmentProbe.Output()
	if err != nil {
		return fmt.Errorf("sanitized child environment: %w", err)
	}
	var environment struct {
		Values   map[string]string `json:"values"`
		MaxProcs int               `json:"gomaxprocs"`
	}
	if err := json.Unmarshal(environmentBytes, &environment); err != nil || environment.MaxProcs != 1 || len(environment.Values) != len(providerEnvironment()) {
		return errors.New("sanitized provider runtime environment mismatch")
	}
	for key, expected := range providerEnvironment() {
		if environment.Values[key] != expected {
			return fmt.Errorf("child fixture environment mismatch: %s", key)
		}
	}
	var vectors []vector
	data, err = os.ReadFile(filepath.Join(root, "decoder-vectors", "vectors.json"))
	if err != nil {
		return err
	}
	if err := json.Unmarshal(data, &vectors); err != nil || len(vectors) != 6 {
		return errors.New("invalid decoder vectors")
	}
	checks := []map[string]any{
		{"check": "manifest-artifact-hashes-and-provider-identity", "status": "PASS"},
		{"check": "sanitized-child-environment", "status": "PASS", "effective_gomaxprocs": environment.MaxProcs, "explicit_keys": len(environment.Values)},
	}
	for _, test := range vectors {
		var parser decoder
		var frames [][]byte
		var rejection error
		for _, fragment := range test.Fragments {
			out, err := parser.feed(fragment)
			frames = append(frames, out...)
			if err != nil {
				rejection = err
				break
			}
		}
		if test.Reject != (rejection != nil) || len(frames) != len(test.Frames) {
			return fmt.Errorf("vector failure: %s", test.Name)
		}
		for index := range frames {
			if !bytes.Equal(frames[index], test.Frames[index]) || !json.Valid(bytes.TrimSpace(frames[index])) {
				return fmt.Errorf("vector content failure: %s", test.Name)
			}
		}
		if !test.Reject && parser.used != 0 {
			return errors.New("decoder has pending bytes after valid vector")
		}
		checks = append(checks, map[string]any{"check": "decoder/" + test.Name, "status": "PASS", "peak_buffer_bytes": parser.peak})
	}
	var payload []string
	data, err = os.ReadFile(filepath.Join(root, "fixtures", "chunks.json"))
	if err != nil {
		return err
	}
	if err := json.Unmarshal(data, &payload); err != nil || len(payload) != 100 {
		return errors.New("invalid normal chunk fixture")
	}
	digest := sha256.Sum256([]byte(strings.Join(payload, "")))
	if hex.EncodeToString(digest[:]) != manifest.Files["fixtures/response.txt"] {
		return errors.New("reconstructed response hash mismatch")
	}
	ordinary, err := launchProvider(root)
	if err != nil {
		return err
	}
	defer ordinary.cleanup()
	for index, scenario := range []string{"normal", "fragmented", "client_request", "cancel", "normal", "stderr", "backpressure", "maximum"} {
		count, err := checkStream(ordinary, int64(index+1), scenario, payload)
		if err != nil {
			return fmt.Errorf("%s: %w", scenario, err)
		}
		checks = append(checks, map[string]any{"check": scenario, "status": "PASS", "logical_chunks": count, "provider_pid": ordinary.cmd.Process.Pid})
	}
	if err := ordinary.shutdown(); err != nil {
		return err
	}
	expectedStderr := bytes.Repeat([]byte("fixture-stderr\n"), 256*1024/15+1)[:256*1024]
	if !bytes.Equal(ordinary.stderr.data, expectedStderr) {
		return errors.New("stderr pressure bytes mismatch")
	}
	checks = append(checks, map[string]any{"check": "stderr-content-and-graceful-shutdown", "status": "PASS", "stderr_bytes": len(ordinary.stderr.data)})
	for _, scenario := range []string{"oversized", "unexpected_exit"} {
		broken, err := launchProvider(root)
		if err != nil {
			return err
		}
		_, err = checkStream(broken, 1, scenario, payload)
		broken.cleanup()
		if err != nil {
			return fmt.Errorf("%s: %w", scenario, err)
		}
		fresh, err := launchProvider(root)
		if err != nil {
			return err
		}
		_, err = checkStream(fresh, 2, "normal", payload)
		if err == nil {
			err = fresh.shutdown()
		}
		fresh.cleanup()
		if err != nil {
			return fmt.Errorf("fresh after %s: %w", scenario, err)
		}
		checks = append(checks, map[string]any{"check": scenario + "/explicit-fresh-session", "status": "PASS", "old_pid": broken.cmd.Process.Pid, "new_pid": fresh.cmd.Process.Pid})
	}
	result := map[string]any{"schema": "mascot-fixture-validation-1", "fixture_version": fixtureVersion, "provider_sha256": binaryHash, "utc": time.Now().UTC().Format(time.RFC3339Nano), "qpc_frequency": frequency(), "checks": checks, "candidate_correctness": "NOT_RUN", "observer_qualification": "PENDING"}
	output := filepath.Join(root, "results", "windows", "raw", "fixture-validation-"+time.Now().UTC().Format("20060102T150405.000000000Z")+".json")
	if err := putJSON(output, result); err != nil {
		return err
	}
	fmt.Println(output)
	return nil
}
