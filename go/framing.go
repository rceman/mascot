package main

// Shared NDJSON logical-frame decoder. Enforces 65536 bytes including LF,
// rejects before buffer growth, validates UTF-8 before the frame callback, and
// captures the receipt QPC at the moment the completing LF is seen, before any
// JSON decoding. The same implementation backs provider stdout, control stdin
// and --decode-vectors mode.

import (
	"errors"
	"unicode/utf8"
)

const frameLimit = 65536

type frameDecoder struct {
	bytes  []byte
	peak   int
	failed bool
}

func newFrameDecoder() *frameDecoder {
	return &frameDecoder{bytes: make([]byte, 0, frameLimit)}
}

func (d *frameDecoder) peakBufferBytes() int { return d.peak }

// feed consumes input bytes. clock is invoked exactly once per completed frame,
// immediately when the terminating LF is appended; frame receives the complete
// logical frame including the LF. The frame callback must not retain the slice.
func (d *frameDecoder) feed(input []byte, clock func() int64, frame func([]byte, int64) error) error {
	if d.failed {
		return errors.New("decoder session already failed")
	}
	for _, b := range input {
		if len(d.bytes) == frameLimit {
			d.failed = true
			return errors.New("logical frame exceeds limit")
		}
		d.bytes = append(d.bytes, b)
		if len(d.bytes) > d.peak {
			d.peak = len(d.bytes)
		}
		if b == '\n' {
			receiptQPC := clock()
			if !utf8.Valid(d.bytes) {
				d.failed = true
				return errors.New("invalid frame UTF-8")
			}
			if err := frame(d.bytes, receiptQPC); err != nil {
				d.failed = true
				return err
			}
			d.bytes = d.bytes[:0]
		}
	}
	return nil
}

func (d *frameDecoder) finish() error {
	if d.failed || len(d.bytes) != 0 {
		return errors.New("failed or incomplete NDJSON stream")
	}
	return nil
}
