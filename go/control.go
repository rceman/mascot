package main

// Control seam (shared): a reader goroutine frames application stdin with the
// shared 65536-byte decoder into a bounded 16-command queue consumed on the UI
// thread; a second goroutine drains the bounded 256-record output queue to
// stdout. Errors emit a bounded error record then trigger the ordinary
// shutdown path. Per-platform files own the reader loop and its cancellation.

import (
	"encoding/json"
	"os"
	"sync/atomic"
	"time"
)

type control struct {
	commands   *boundedQueue[map[string]json.RawMessage]
	records    *boundedQueue[string]
	stop       atomic.Bool
	readerDone chan struct{}
	writerDone chan struct{}
	armed      bool
	plat       controlPlat
}

func (c *control) pushErrorRecord(line string) {
	if c.records != nil {
		c.records.tryPush(line)
	}
}

func (c *control) requestShutdownFromReader(wake func()) {
	c.commands.push(map[string]json.RawMessage{
		"command": json.RawMessage(`"shutdown"`),
		"token":   json.RawMessage("null"),
	})
	wake()
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
	c.plat = newControlPlat()
	c.armed = true
	go c.writerRun()
	c.startReader(ui)
}

func (c *control) joinBounded(done chan struct{}, deadline time.Time) bool {
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
	}
}

func (c *control) stopAll() {
	c.stop.Store(true)
	if c.commands != nil {
		c.commands.close()
	}
	deadline := time.Now().Add(2 * time.Second)
	if c.writerDone != nil {
		if !c.joinBounded(c.writerDone, deadline) {
			os.Stderr.WriteString("control writer did not finish within teardown bound\n")
		}
	}
	if c.readerDone != nil {
		if !c.stopReader(deadline) {
			os.Stderr.WriteString("control reader did not finish within teardown bound\n")
		}
	}
}
