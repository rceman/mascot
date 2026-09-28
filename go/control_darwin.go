package main

// macOS control reader: nonblocking stdin plus a stop pipe, multiplexed with
// syscall select(2) so teardown wakes the reader deterministically inside the
// shared 2 s bound.

import (
	"encoding/json"
	"errors"
	"os"
	"syscall"
	"time"

	"golang.org/x/sys/unix"
)

type controlPlat struct {
	stopRead  int
	stopWrite int
}

func newControlPlat() controlPlat {
	plat := controlPlat{stopRead: -1, stopWrite: -1}
	var fds [2]int
	if err := syscall.Pipe(fds[:]); err != nil {
		os.Stderr.WriteString("control stop pipe creation failed\n")
		return plat
	}
	plat.stopRead, plat.stopWrite = fds[0], fds[1]
	return plat
}

func (c *control) startReader(ui *UI) {
	go c.readerRun(ui.postControlWake)
}

func (c *control) readerRun(wake func()) {
	defer close(c.readerDone)
	stopRead := c.plat.stopRead
	_ = unix.SetNonblock(0, true)
	if stopRead >= 0 {
		_ = unix.SetNonblock(stopRead, true)
	}
	decoder := newFrameDecoder()
	buf := make([]byte, 4096)
	var scratch [4]byte
	for {
		var readSet unix.FdSet
		readSet.Set(0)
		if stopRead >= 0 {
			readSet.Set(stopRead)
		}
		nfds := 1
		if stopRead >= 0 && stopRead+1 > nfds {
			nfds = stopRead + 1
		}
		n, err := unix.Select(nfds, &readSet, nil, nil, nil)
		if err != nil {
			if errors.Is(err, unix.EINTR) {
				continue
			}
			msg, _ := json.Marshal(map[string]any{
				"token": nil, "ok": false, "error": "control poll failed: " + err.Error(),
			})
			c.pushErrorRecord(string(msg))
			c.requestShutdownFromReader(wake)
			return
		}
		if n <= 0 {
			continue
		}
		if stopRead >= 0 && readSet.IsSet(stopRead) {
			_, _ = unix.Read(stopRead, scratch[:])
			return
		}
		if !readSet.IsSet(0) {
			continue
		}
		count, rerr := unix.Read(0, buf)
		if count > 0 {
			feedErr := decoder.feed(buf[:count], func() int64 { return 0 }, func(frame []byte, _ int64) error {
				var cmd map[string]json.RawMessage
				if jerr := json.Unmarshal(frame, &cmd); jerr != nil {
					return jerr
				}
				if !c.commands.push(cmd) {
					return errors.New("control queue closed")
				}
				wake()
				return nil
			})
			if feedErr != nil {
				msg, _ := json.Marshal(map[string]any{
					"token": nil, "ok": false,
					"error": "invalid control frame: " + feedErr.Error(),
				})
				c.pushErrorRecord(string(msg))
				c.requestShutdownFromReader(wake)
				return
			}
		}
		if count == 0 {
			if decoder.finish() != nil {
				msg, _ := json.Marshal(map[string]any{
					"token": nil, "ok": false, "error": "incomplete control frame",
				})
				c.pushErrorRecord(string(msg))
			}
			c.requestShutdownFromReader(wake)
			return
		}
		if rerr != nil {
			if errors.Is(rerr, unix.EINTR) || errors.Is(rerr, unix.EAGAIN) {
				continue
			}
			if c.stop.Load() {
				return
			}
			msg, _ := json.Marshal(map[string]any{
				"token": nil, "ok": false, "error": "control input read failed",
			})
			c.pushErrorRecord(string(msg))
			c.requestShutdownFromReader(wake)
			return
		}
	}
}

func (c *control) stopReader(deadline time.Time) bool {
	if c.plat.stopWrite >= 0 {
		_, _ = unix.Write(c.plat.stopWrite, []byte{1})
		_ = unix.Close(c.plat.stopWrite)
		c.plat.stopWrite = -1
	}
	ok := c.joinBounded(c.readerDone, deadline)
	if c.plat.stopRead >= 0 {
		_ = unix.Close(c.plat.stopRead)
		c.plat.stopRead = -1
	}
	return ok
}
