package main

import (
	"encoding/json"
	"testing"
	"time"
)

func testShared() (*sharedState, *boundedQueue[providerEvent], *boundedQueue[string], *boundedQueue[workItem]) {
	return newSharedState(),
		newBoundedQueue[providerEvent](64),
		newBoundedQueue[string](256),
		newBoundedQueue[workItem](16)
}

func activate(shared *sharedState, id uint64, expected uint64) {
	shared.mu.Lock()
	shared.generation = 1
	shared.protocol = &requestProtocol{id: id, expectedChunks: expected, phase: phaseAwaitStart}
	shared.mu.Unlock()
}

func startFrame(id, chunks uint64) []byte {
	f, _ := json.Marshal(map[string]any{
		"type": "start", "id": id, "chunks": chunks, "qpc_frequency": qpcFrequency(),
	})
	return f
}

func chunkFrame(id, seq uint64, text string) []byte {
	f, _ := json.Marshal(map[string]any{
		"type": "chunk", "id": id, "seq": seq, "text": text, "emit_qpc": "00000000000000000100",
	})
	return f
}

func feed(shared *sharedState, events *boundedQueue[providerEvent], records *boundedQueue[string],
	work *boundedQueue[workItem], frame []byte) error {
	return handleFrame(frame, 7, 1, shared, events, records, work, func() {})
}

func TestNormalSequenceAccepted(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	if err := feed(shared, events, records, work, startFrame(7, 2)); err != nil {
		t.Fatal(err)
	}
	if err := feed(shared, events, records, work, chunkFrame(7, 0, "a")); err != nil {
		t.Fatal(err)
	}
	if err := feed(shared, events, records, work, chunkFrame(7, 1, "b")); err != nil {
		t.Fatal(err)
	}
	if err := feed(shared, events, records, work, []byte(`{"type":"complete","id":7}`)); err != nil {
		t.Fatal(err)
	}
	shared.mu.Lock()
	if shared.protocol.phase != phaseTerminal || shared.protocol.nextSeq != 2 {
		t.Fatalf("phase=%v nextSeq=%d", shared.protocol.phase, shared.protocol.nextSeq)
	}
	shared.mu.Unlock()
	terminals, chunks := 0, 0
	for {
		ev, ok := events.pop()
		if !ok {
			break
		}
		switch ev.kind {
		case evChunk:
			chunks++
			if ev.seq >= 2 {
				t.Fatalf("seq %d", ev.seq)
			}
		case evTerminal:
			terminals++
			if ev.termKind != "complete" || ev.lastSeq != 1 {
				t.Fatalf("terminal %v %d", ev.termKind, ev.lastSeq)
			}
		}
	}
	if chunks != 2 || terminals != 1 {
		t.Fatalf("chunks=%d terminals=%d", chunks, terminals)
	}
	if _, ok := records.pop(); !ok {
		t.Fatal("missing frame_received record")
	}
}

func TestWrongIDRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	if err := feed(shared, events, records, work, startFrame(7, 2)); err != nil {
		t.Fatal(err)
	}
	if err := feed(shared, events, records, work, chunkFrame(8, 0, "a")); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestDuplicateSeqRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	feed(shared, events, records, work, startFrame(7, 2))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	if err := feed(shared, events, records, work, chunkFrame(7, 0, "a")); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestSkippedSeqRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	feed(shared, events, records, work, startFrame(7, 2))
	if err := feed(shared, events, records, work, chunkFrame(7, 1, "b")); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestPrematureCompleteRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	feed(shared, events, records, work, startFrame(7, 2))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	if err := feed(shared, events, records, work, []byte(`{"type":"complete","id":7}`)); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestDuplicateTerminalRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 1)
	feed(shared, events, records, work, startFrame(7, 1))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	if err := feed(shared, events, records, work, []byte(`{"type":"complete","id":7}`)); err != nil {
		t.Fatal(err)
	}
	if err := feed(shared, events, records, work, []byte(`{"type":"complete","id":7}`)); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestTerminalClaimOnce(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 3)
	feed(shared, events, records, work, startFrame(7, 3))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	feed(shared, events, records, work, chunkFrame(7, 1, "b"))
	shared.mu.Lock()
	id, seq, ok := shared.protocol.claimTerminal()
	if !ok || id != 7 || seq != 1 {
		t.Fatalf("claim = %d,%d,%v", id, seq, ok)
	}
	if _, _, ok := shared.protocol.claimTerminal(); ok {
		t.Fatal("second claim should fail")
	}
	shared.mu.Unlock()
}

func TestFrequencyMismatchRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	f, _ := json.Marshal(map[string]any{
		"type": "start", "id": 7, "chunks": 2, "qpc_frequency": 12345,
	})
	if err := feed(shared, events, records, work, f); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestCancelAfterOneChunkPermitsReuse(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	feed(shared, events, records, work, startFrame(7, 2))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	shared.mu.Lock()
	shared.protocol.cancelRequested = true
	shared.mu.Unlock()
	if err := feed(shared, events, records, work, []byte(`{"type":"cancelled","id":7,"last_seq":0}`)); err != nil {
		t.Fatal(err)
	}
	activate(shared, 8, 1)
	feed(shared, events, records, work, startFrame(8, 1))
	feed(shared, events, records, work, chunkFrame(8, 0, "z"))
	if err := feed(shared, events, records, work, []byte(`{"type":"complete","id":8}`)); err != nil {
		t.Fatal(err)
	}
	shared.mu.Lock()
	defer shared.mu.Unlock()
	if shared.protocol.phase != phaseTerminal {
		t.Fatal("expected terminal")
	}
}

func TestCancelledWithoutRequestRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	feed(shared, events, records, work, startFrame(7, 2))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	if err := feed(shared, events, records, work, []byte(`{"type":"cancelled","id":7,"last_seq":0}`)); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestClientRequestAnswersOnce(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 1)
	feed(shared, events, records, work, startFrame(7, 1))
	req := []byte(`{"type":"client_request","id":7,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}`)
	if err := feed(shared, events, records, work, req); err != nil {
		t.Fatal(err)
	}
	item, ok := work.pop()
	if !ok || item.kind != workClientResponse || item.generation != 1 || item.id != 7 || item.requestID != 1 {
		t.Fatalf("work item = %+v", item)
	}
	if err := feed(shared, events, records, work, req); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestClientRequestBeforeStartRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 1)
	req := []byte(`{"type":"client_request","id":7,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}`)
	if err := feed(shared, events, records, work, req); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestClientRequestAfterTerminalRejected(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 1)
	feed(shared, events, records, work, startFrame(7, 1))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	feed(shared, events, records, work, []byte(`{"type":"complete","id":7}`))
	req := []byte(`{"type":"client_request","id":7,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}`)
	if err := feed(shared, events, records, work, req); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestShutdownAckRequiresRequestAndIsOnce(t *testing.T) {
	shared, events, records, work := testShared()
	shared.mu.Lock()
	shared.generation = 1
	shared.mu.Unlock()
	ack := []byte(`{"type":"shutdown_ack"}`)
	if err := feed(shared, events, records, work, ack); err == nil {
		t.Fatal("expected rejection")
	}
	shared.mu.Lock()
	shared.shutdownSent = true
	shared.mu.Unlock()
	if err := feed(shared, events, records, work, ack); err != nil {
		t.Fatal(err)
	}
	if err := feed(shared, events, records, work, ack); err == nil {
		t.Fatal("expected rejection")
	}
}

func TestShutdownAckAcceptedMidStream(t *testing.T) {
	shared, events, records, work := testShared()
	activate(shared, 7, 2)
	feed(shared, events, records, work, startFrame(7, 2))
	feed(shared, events, records, work, chunkFrame(7, 0, "a"))
	shared.mu.Lock()
	shared.shutdownSent = true
	shared.mu.Unlock()
	if err := feed(shared, events, records, work, []byte(`{"type":"shutdown_ack"}`)); err != nil {
		t.Fatal(err)
	}
	shared.mu.Lock()
	defer shared.mu.Unlock()
	if !shared.shutdownAck {
		t.Fatal("expected ack")
	}
}

func TestEventQueueBackpressureBlocksReader(t *testing.T) {
	shared := newSharedState()
	events := newBoundedQueue[providerEvent](1)
	records := newBoundedQueue[string](256)
	work := newBoundedQueue[workItem](16)
	activate(shared, 7, 2)
	if err := feed(shared, events, records, work, startFrame(7, 2)); err != nil {
		t.Fatal(err)
	}
	events.push(providerEvent{kind: evStarted, generation: 1, pid: 1})
	done := make(chan error, 1)
	go func() {
		done <- handleFrame(chunkFrame(7, 0, "a"), 7, 1, shared, events, records, work, func() {})
	}()
	select {
	case <-done:
		t.Fatal("expected blocked push")
	case <-time.After(20 * time.Millisecond):
	}
	if ev, ok := events.pop(); !ok || ev.kind != evStarted {
		t.Fatalf("pop = %+v", ev)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("blocked push did not complete")
	}
	ev, ok := events.pop()
	if !ok || ev.kind != evChunk || ev.id != 7 || ev.seq != 0 || ev.text != "a" {
		t.Fatalf("chunk event = %+v", ev)
	}
	if _, ok := events.pop(); ok {
		t.Fatal("unexpected extra event")
	}
}

func TestAppendBoundedEnforcesLimitAndNul(t *testing.T) {
	response := "A"
	if err := appendBounded(&response, "é", 3); err != nil {
		t.Fatal(err)
	}
	if response != "Aé" {
		t.Fatalf("response = %q", response)
	}
	if err := appendBounded(&response, "B", 3); err == nil {
		t.Fatal("expected bound rejection")
	}
	if response != "Aé" {
		t.Fatalf("response = %q", response)
	}
	if err := appendBounded(&response, "x\x00y", 10); err == nil {
		t.Fatal("expected NUL rejection")
	}
}

func TestStaleGenerationEventsDoNotMatch(t *testing.T) {
	if eventMatches(1, 7, 2, 7) {
		t.Fatal("stale generation matched")
	}
	if !eventMatches(2, 7, 2, 7) {
		t.Fatal("current generation did not match")
	}
	if eventMatches(2, 8, 2, 7) {
		t.Fatal("wrong request matched")
	}
}

func TestFontHeightScalesWithDPI(t *testing.T) {
	if fontHeight(96) != -16 || fontHeight(192) != -32 {
		t.Fatal("font height")
	}
}

