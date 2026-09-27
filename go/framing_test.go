package main

import "testing"

func TestSplitMultibyteProducesExactFrames(t *testing.T) {
	d := newFrameDecoder()
	var calls int64
	var frames [][]byte
	clock := func() int64 { calls++; return calls }
	collect := func(frame []byte, _ int64) error {
		frames = append(frames, append([]byte(nil), frame...))
		return nil
	}
	if err := d.feed([]byte("{\"text\":\"\xc4"), clock, collect); err != nil {
		t.Fatal(err)
	}
	if err := d.feed([]byte("\x80\"}\n{\"type\":\"complete\"}\n"), clock, collect); err != nil {
		t.Fatal(err)
	}
	if err := d.finish(); err != nil {
		t.Fatal(err)
	}
	if calls != 2 {
		t.Fatalf("calls = %d", calls)
	}
	if len(frames) != 2 {
		t.Fatalf("frames = %d", len(frames))
	}
	if string(frames[0]) != "{\"text\":\"Ā\"}\n" {
		t.Fatalf("frame0 = %q", frames[0])
	}
	if string(frames[1]) != "{\"type\":\"complete\"}\n" {
		t.Fatalf("frame1 = %q", frames[1])
	}
}

func TestMaximumFrameAcceptedAtLimit(t *testing.T) {
	d := newFrameDecoder()
	frames := 0
	collect := func(frame []byte, _ int64) error {
		frames++
		return nil
	}
	payload := make([]byte, frameLimit-1)
	for i := range payload {
		payload[i] = 'x'
	}
	if err := d.feed(payload, func() int64 { return 0 }, collect); err != nil {
		t.Fatal(err)
	}
	if err := d.feed([]byte("\n"), func() int64 { return 0 }, collect); err != nil {
		t.Fatal(err)
	}
	if d.peakBufferBytes() != frameLimit {
		t.Fatalf("peak = %d", d.peakBufferBytes())
	}
	if frames != 1 {
		t.Fatalf("frames = %d", frames)
	}
}

func TestOversizedRejectedBeforeGrowth(t *testing.T) {
	d := newFrameDecoder()
	frames := 0
	collect := func(frame []byte, _ int64) error {
		frames++
		return nil
	}
	payload := make([]byte, frameLimit+1)
	for i := range payload {
		payload[i] = 'x'
	}
	if err := d.feed(payload, func() int64 { return 0 }, collect); err == nil {
		t.Fatal("expected rejection")
	}
	if d.peakBufferBytes() != frameLimit {
		t.Fatalf("peak = %d", d.peakBufferBytes())
	}
	if frames != 0 {
		t.Fatalf("frames = %d", frames)
	}
	if err := d.feed([]byte("\n"), func() int64 { return 0 }, collect); err == nil {
		t.Fatal("expected sticky failure")
	}
	if err := d.finish(); err == nil {
		t.Fatal("expected finish failure")
	}
}

func TestPartialFrameFailsFinish(t *testing.T) {
	d := newFrameDecoder()
	if err := d.feed([]byte("{\"partial\":"), func() int64 { return 0 }, func([]byte, int64) error { return nil }); err != nil {
		t.Fatal(err)
	}
	if err := d.finish(); err == nil {
		t.Fatal("expected finish failure")
	}
}

func TestInvalidUTF8RejectedOnlyAtFrameEnd(t *testing.T) {
	d := newFrameDecoder()
	frames := 0
	collect := func(frame []byte, _ int64) error {
		frames++
		return nil
	}
	if err := d.feed([]byte("{\"bad\":\"\xff"), func() int64 { return 0 }, collect); err != nil {
		t.Fatal(err)
	}
	if err := d.feed([]byte("\"}\n"), func() int64 { return 0 }, collect); err == nil {
		t.Fatal("expected UTF-8 rejection")
	}
	if frames != 0 {
		t.Fatalf("frames = %d", frames)
	}
}
