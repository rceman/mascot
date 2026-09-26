# Mock Provider Contract v0.1

Both Rust and Zig candidates must use the same benchmark-provider executable and this exact logical protocol.

The provider is excluded from application-owned memory totals because it is byte-identical for both candidates. Any candidate-specific helper remains part of that candidate's application total.

## 1. Transport and lifetime

- One persistent child process per benchmark session.
- stdin and stdout carry UTF-8 newline-delimited JSON (NDJSON), one complete JSON object per logical frame.
- stderr is an independent byte stream and must always be drained.
- The child remains alive across multiple requests until explicit shutdown or an injected unexpected-exit test.
- Candidate code must not restart the child between ordinary benchmark requests.
- Candidate code must not kill the child as its normal cancellation mechanism.
- Cooperative cancellation is the normal path.
- On shell shutdown, the child must be terminated/reaped within the harness timeout.

## 2. Frame limits

- Maximum accepted logical stdout frame: 65,536 bytes including the trailing newline.
- Maximum accepted logical stdin frame: 65,536 bytes including the trailing newline.
- The over-limit test sends a 65,537-byte logical frame.
- Candidate code must reject an over-limit frame without unbounded allocation or process hang.

## 3. Required messages

### Client -> provider: request

Example:

    {"type":"request","id":17,"prompt":"hello"}

Rules:

- id is a positive integer unique among active requests.
- prompt is UTF-8.
- One ordinary request is active at a time in v0.1.

### Provider -> client: start

    {"type":"start","id":17,"chunks":100}

### Provider -> client: chunk

    {"type":"chunk","id":17,"seq":0,"text":"..."}

Rules:

- seq is zero-based.
- Normal response contains exactly 100 chunk frames.
- Sequence must be preserved.
- Client may coalesce visual presentation but may not reorder or drop logical chunks.

### Provider -> client: complete

    {"type":"complete","id":17}

### Client -> provider: cancel

    {"type":"cancel","id":17}

### Provider -> client: cancelled

    {"type":"cancelled","id":17,"last_seq":49}

The canonical cancellation scenario sends cancel immediately after the client has accepted logical chunk 49. The provider must stop further chunks for that request and return cancelled.

### Provider -> client: client_request

    {"type":"client_request","id":17,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}

### Client -> provider: client_response

    {"type":"client_response","request_id":1,"result":{"accepted":true}}

The provider does not continue that test case until the correct response is received.

### Client -> provider: shutdown

    {"type":"shutdown"}

The provider acknowledges with:

    {"type":"shutdown_ack"}

and exits zero.

## 4. Normal response payload

The fixture owns the canonical payload file.

Requirements:

- exactly 100 logical chunk frames
- deterministic text and byte count
- includes ASCII, Latvian, Cyrillic, combining characters, and multi-codepoint emoji across the complete response
- at least one UTF-8 multi-byte sequence is intentionally split across physical pipe writes
- exact SHA-256 of the reconstructed text is recorded in the fixture manifest

The implementation agent must generate and freeze the manifest once; both candidates consume the same manifest unchanged.

## 5. Scheduling

Normal streaming:

- 100 logical chunks
- nominal inter-chunk interval: 10 ms
- timestamps emitted by the mock harness use the Windows QPC-derived benchmark clock in Stage A
- test harness records sequence ID and emission timestamp

Scheduling jitter is measured, not treated as application latency.

## 6. Physical write patterns

Logical NDJSON frames are intentionally delivered using several physical write modes:

1. whole-frame writes
2. frame split into small writes
3. multiple complete frames coalesced into one write
4. split at a multi-byte UTF-8 boundary
5. split immediately before the newline delimiter

The exact deterministic pattern is defined in the fixture manifest.

Candidates must parse the byte stream correctly rather than assume one read equals one frame.

## 7. stderr pressure

For the stderr-drain case, the provider emits a deterministic 256 KiB diagnostic stream while stdout continues normally.

The shell must not deadlock, block UI interaction, or silently stop reading stdout.

stderr content does not need to be rendered in the prototype UI.

## 8. Backpressure case

A separate case emits 256 valid chunk frames as quickly as the provider can write them, subject to normal pipe backpressure.

Requirements:

- bounded client queue
- no unbounded memory growth
- no lost/reordered logical frames
- UI remains responsive
- coalesced redraw is allowed and encouraged

This case is diagnostic and is not used for the headline 10 ms streamed-response latency numbers.

## 9. Unexpected-exit case

After the start frame and a deterministic number of chunks, the provider exits with the fixture-defined non-zero code.

The shell must:

- detect EOF/process exit
- stop waiting for more response data
- surface a deterministic provider-failed state
- remain usable
- leave no orphaned child/process handles

## 10. Frame-limit cases

### Maximum valid

Provider emits one valid logical frame exactly at the configured maximum size. The client must accept it.

### Oversized

Provider emits one logical frame one byte above the configured maximum. The client must reject the frame/request deterministically, keep memory bounded, and either keep or restart the provider according to the shared harness policy.

Both candidates must use the same policy.

## 11. State retention

For benchmark equality:

- The response area holds only the current request plus a fixed fixture of four prior short messages.
- Starting a new normal request replaces the previous current-response body after completion/cancellation.
- Hiding the composer does not cancel the active request.
- Reopening the composer shows the current in-progress/completed response.
- Explicit cancel is the only normal user cancellation action.
- No database or unbounded transcript is used.

## 12. Fixture deliverables

The implementation task must create one shared provider under the benchmark area, not one provider per language.

It must include:

- fixture version
- exact response text/payload
- reconstructed response SHA-256
- exact frame sequence
- exact physical-write fragmentation plan
- exact failure exit code
- exact stderr fixture
- exact frame limits
- executable build/run instructions

Once both candidate implementations begin measurement, fixture changes require invalidating and rerunning affected measurements.
