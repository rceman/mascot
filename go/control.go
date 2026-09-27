package main

// Control seam: a locked-OS-thread goroutine frames application stdin with the
// shared 65536-byte decoder into a bounded 16-command queue consumed on the UI
// thread; a second goroutine drains the bounded 256-record output queue to
// stdout. Errors emit a bounded error record then trigger the ordinary
// shutdown path. Teardown cancels the blocked stdin read via
// CancelSynchronousIo on the reader's own published thread handle.

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"runtime"
	"sync/atomic"
	"time"

	"golang.org/x/sys/windows"
)

type control struct {
	commands    *boundedQueue[map[string]json.RawMessage]
	records     *boundedQueue[string]
	stop        atomic.Bool
	readerDone  chan struct{}
	writerDone  chan struct{}
	threadReady chan windows.Handle
	armed       bool
}

func postControl(hwnd uintptr) {
	postMessage(hwnd, wmAPPCONTROL, 0, 0)
}

func (c *control) pushErrorRecord(line string) {
	if c.records != nil {
		c.records.tryPush(line)
	}
}

func (c *control) requestShutdownFromReader(hwnd uintptr) {
	c.commands.push(map[string]json.RawMessage{
		"command": json.RawMessage(`"shutdown"`),
		"token":   json.RawMessage("null"),
	})
	postControl(hwnd)
}

func (c *control) readerRun(hwnd uintptr) {
	defer close(c.readerDone)
	runtime.LockOSThread()
	// Publish a real handle to this OS thread so stop() can cancel a blocked
	// synchronous ReadFile on stdin.
	var dup windows.Handle
	if err := windows.DuplicateHandle(
		windows.CurrentProcess(), windows.CurrentThread(),
		windows.CurrentProcess(), &dup, 0, false, duplicateSAMEACCESS,
	); err != nil {
		c.threadReady <- 0
	} else {
		c.threadReady <- dup
	}
	decoder := newFrameDecoder()
	buf := make([]byte, 4096)
	for {
		if c.stop.Load() {
			return
		}
		n, err := os.Stdin.Read(buf)
		if n > 0 {
			feedErr := decoder.feed(buf[:n], func() int64 { return 0 }, func(frame []byte, _ int64) error {
				var cmd map[string]json.RawMessage
				if jerr := json.Unmarshal(frame, &cmd); jerr != nil {
					return jerr
				}
				if !c.commands.push(cmd) {
					return errors.New("control queue closed")
				}
				postControl(hwnd)
				return nil
			})
			if feedErr != nil {
				msg, _ := json.Marshal(map[string]any{
					"token": nil, "ok": false,
					"error": "invalid control frame: " + feedErr.Error(),
				})
				c.pushErrorRecord(string(msg))
				c.requestShutdownFromReader(hwnd)
				return
			}
		}
		if err != nil {
			if errors.Is(err, io.EOF) {
				if decoder.finish() != nil {
					msg, _ := json.Marshal(map[string]any{
						"token": nil, "ok": false, "error": "incomplete control frame",
					})
					c.pushErrorRecord(string(msg))
				}
				c.requestShutdownFromReader(hwnd)
				return
			}
			if c.stop.Load() {
				return
			}
			msg, _ := json.Marshal(map[string]any{
				"token": nil, "ok": false, "error": "control input read failed",
			})
			c.pushErrorRecord(string(msg))
			c.requestShutdownFromReader(hwnd)
			return
		}
		if n == 0 {
			// Defensive: avoid a hot loop on a 0,nil read.
			time.Sleep(time.Millisecond)
		}
	}
}

func (c *control) writerRun() {
	defer close(c.writerDone)
	for {
		line, ok := c.records.waitPop()
		if !ok {
			return
		}
		if _, err := os.Stdout.WriteString(line + "\n"); err != nil {
			return
		}
	}
}

func (c *control) arm(ui *UI, enabled bool) {
	if !enabled || ui.records == nil {
		return
	}
	c.commands = ui.commands
	c.records = ui.records
	c.readerDone = make(chan struct{})
	c.writerDone = make(chan struct{})
	c.threadReady = make(chan windows.Handle, 1)
	c.armed = true
	go c.writerRun()
	go c.readerRun(ui.mascotHwnd)
}

func (c *control) joinBounded(done chan struct{}, deadline time.Time, thread windows.Handle) bool {
	for {
		select {
		case <-done:
			return true
		default:
		}
		remaining := time.Until(deadline)
		if remaining <= 0 {
			return false
		}
		wait := remaining
		if wait > 20*time.Millisecond {
			wait = 20 * time.Millisecond
		}
		timer := time.NewTimer(wait)
		select {
		case <-done:
			timer.Stop()
			return true
		case <-timer.C:
		}
		if thread != 0 {
			procCancelSynchronousIo.Call(uintptr(thread))
		}
	}
}

func (c *control) stopAll() {
	c.stop.Store(true)
	if c.commands != nil {
		c.commands.close()
	}
	deadline := time.Now().Add(2 * time.Second)
	if c.writerDone != nil {
		if !c.joinBounded(c.writerDone, deadline, 0) {
			os.Stderr.WriteString("control writer did not finish within teardown bound\n")
		}
	}
	if c.readerDone != nil {
		var thread windows.Handle
		select {
		case thread = <-c.threadReady:
		case <-time.After(500 * time.Millisecond):
		}
		if !c.joinBounded(c.readerDone, deadline, thread) {
			os.Stderr.WriteString("control reader did not finish within teardown bound\n")
		}
		if thread != 0 {
			windows.CloseHandle(thread)
		}
	}
}
