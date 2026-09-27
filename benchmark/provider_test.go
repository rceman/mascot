package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestExactFrameLimits(t *testing.T) {
	for _, id := range []int64{1, 17, 9223372036854775807} {
		for _, size := range []int{frameLimit, frameLimit + 1} {
			frame := chunk(id, 0, strings.Repeat("x", size-len(chunk(id, 0, ""))))
			if len(frame) != size || !json.Valid(bytes.TrimSpace(frame)) {
				t.Fatal("incorrect serialized frame size or invalid JSON")
			}
			var parser decoder
			var rejected error
			var frames [][]byte
			for _, item := range frame {
				out, err := parser.feed([]byte{item})
				frames = append(frames, out...)
				if err != nil {
					rejected = err
					break
				}
			}
			if (size > frameLimit) != (rejected != nil) || parser.peak > frameLimit {
				t.Fatal("incorrect bounded decoder limit")
			}
			if size == frameLimit && (len(frames) != 1 || !bytes.Equal(frames[0], frame)) {
				t.Fatal("valid maximum frame lost")
			}
		}
	}
}

func TestDecoderUTF8AndReuse(t *testing.T) {
	var parser decoder
	first := []byte("{\"text\":\"Ā\"}\n")
	for index := range first {
		frames, err := parser.feed(first[index : index+1])
		if err != nil || (index+1 < len(first) && len(frames) != 0) {
			t.Fatal("UTF-8 fragment decoded too early", err)
		}
		if index+1 == len(first) && (len(frames) != 1 || !bytes.Equal(frames[0], first)) {
			t.Fatal("UTF-8 frame not reconstructed")
		}
	}
	frames, err := parser.feed(append(append([]byte{}, first...), first...))
	if err != nil || len(frames) != 2 || parser.used != 0 {
		t.Fatal("decoder failed reuse/coalescing", err)
	}
	if _, err := parser.feed([]byte{0xff, '\n'}); err == nil {
		t.Fatal("decoder accepted invalid UTF-8")
	}
}
