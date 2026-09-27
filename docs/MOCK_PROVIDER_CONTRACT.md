# Mock Provider Contract v0.2

The Rust, Zig, and Go candidates must use the same benchmark-provider executable and this exact logical protocol.

The provider is excluded from application-owned memory totals because it is byte-identical for all candidates. Any candidate-specific helper remains part of that candidate's application total.

## 1. Fixture freeze gate

The common fixture/harness is prepared and frozen **before any candidate application implementation begins**.

The frozen manifest must include:

- fixture version
- scenario list
- exact response text/payloads
- reconstructed response SHA-256 values
- exact logical frame sequences
- exact physical-write fragmentation plan
- direct decoder-fragment vectors
- exact cancellation barrier behavior
- exact exceptional-session recovery policy
- shutdown timeout
- cancellation timeout
- frame limits
- stderr fixture
- failure exit codes
- expected terminal events
- text/visual fixture version
- benchmark mascot asset ID/hash and logical dimensions
- provider executable identity and command-line arguments
- provider working directory
- explicit sanitized fixture-specific environment

The provider launch configuration is independent of candidate-only runtime/allocator tuning. The shared provider must not inherit candidate-specific settings that would change its behavior (for example Go GC/debug settings when the fixture itself happens to be implemented in Go).

Do not record secrets or dump the entire inherited environment. Freeze only the explicit fixture environment required for deterministic behavior.

Any later change increments the fixture version and invalidates every affected earlier result, including results collected before another candidate was implemented or measured.

## 2. Transport and lifetime

- One persistent child process per ordinary benchmark provider session.
- stdin and stdout carry UTF-8 newline-delimited JSON (NDJSON), one complete JSON object per logical frame.
- stderr is an independent byte stream and must always be drained.
- The child remains alive across ordinary completion and cooperative cancellation.
- Candidate code must not restart the child between ordinary requests.
- Candidate code must not kill the child as its normal cancellation mechanism.
- Cooperative cancellation is the normal path.
- Exceptional protocol cases may invalidate the session only where this contract explicitly says so.
- On shell shutdown, the child must be terminated/reaped within the shared shutdown timeout.

Shared v0.2 timeout defaults, frozen into the manifest:

- shutdown_timeout_ms: 2000
- cancel_timeout_ms: 1000

## 3. Frame limits

- Maximum accepted logical stdout frame: 65,536 bytes including the trailing newline.
- Maximum accepted logical stdin frame: 65,536 bytes including the trailing newline.
- The over-limit test sends a 65,537-byte logical frame.
- Candidate code must reject an over-limit frame without unbounded allocation or process hang.

## 4. Required messages

### Client -> provider: request

Example:

    {"type":"request","id":17,"prompt":"hello"}

Rules:

- id is a positive integer unique among active requests.
- prompt is UTF-8.
- One ordinary request is active at a time in v0.2.

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

### Canonical cancellation barrier

For the canonical cancellation case only:

1. The provider emits chunks 0 through 49.
2. After chunk 49 is emitted, the provider pauses and does not emit chunk 50.
3. The client decodes and accepts chunk 49.
4. The client sends cancel for that request.
5. The provider emits exactly one cancelled terminal frame with last_seq 49.
6. The provider emits neither chunk 50 nor complete for that request.
7. The same child process remains alive and must successfully serve the next ordinary request.

If cancel is not received within cancel_timeout_ms, the fixture fails the scenario according to the manifest.

This barrier exists only to make the comparative benchmark deterministic. It is not an assumption about real provider cancellation races.

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

## 5. Normal response payload

The fixture owns the canonical payload file.

Requirements:

- exactly 100 logical chunk frames
- deterministic text and byte count
- includes ASCII
- includes Latvian
- includes Cyrillic
- includes combining characters
- includes multi-codepoint emoji
- includes the shared mixed-direction Arabic rendering fixture
- at least one UTF-8 multi-byte sequence is intentionally split across physical provider writes
- exact SHA-256 of reconstructed text is recorded in the manifest

The response view must satisfy the visual correctness cases in docs/TEXT_FIXTURES.md.

## 6. Scheduling and emission timestamp

Normal streaming:

- 100 logical chunks
- nominal inter-chunk interval: 10 ms
- Stage A uses the shared Windows QPC-derived benchmark clock
- each logical chunk has a sequence ID and emission timestamp

The manifest defines the emission timestamp as the timestamp captured **immediately before the first physical write attempt containing any bytes of that logical frame**.

Scheduling jitter is reported separately and is not silently attributed to application rendering latency.

## 7. Physical write patterns and decoder vectors

Logical NDJSON frames are delivered using deterministic physical write patterns:

1. whole-frame writes
2. one frame split into small writes
3. multiple complete frames coalesced into one write
4. split at a multi-byte UTF-8 boundary
5. split immediately before the newline delimiter

The manifest freezes the exact write plan.

Important: provider write fragmentation does not prove the OS delivered matching fragmented reads.

Therefore all three candidates must also run the same direct decoder-fragment vectors, which feed the parser exact byte fragments independent of pipe read behavior.

P3 passes only if:

- the direct fragment vectors pass, and
- the end-to-end physical-write scenario passes.

## 8. stderr pressure

For the stderr-drain case, the provider emits a deterministic 256 KiB diagnostic stream while stdout continues normally.

The shell must not deadlock, block UI interaction, or silently stop reading stdout.

stderr content does not need to be rendered.

## 9. Backpressure case

A separate case emits 256 valid chunk frames as quickly as the provider can write them, subject to pipe backpressure.

Requirements:

- bounded client queue
- no unbounded memory growth
- no lost/reordered logical frames
- UI remains responsive
- coalesced redraw is allowed and encouraged

This case is diagnostic and is not used for headline 10 ms streamed-response latency.

## 10. Unexpected-exit case

After start and a deterministic number of chunks, the provider exits with the fixture-defined non-zero code.

The shell must:

- detect EOF/process exit
- stop waiting for more response data
- surface a deterministic provider-failed state
- remain usable
- leave no orphaned child/process handles

The session ends. The next explicit request may start a fresh provider session according to the manifest.

## 11. Frame-limit cases

### Maximum valid

Provider emits one valid logical frame exactly at the configured maximum size.

The client must accept it.

### Oversized

Provider emits one logical frame one byte above the configured maximum size.

Required recovery policy:

1. The client fails the current request.
2. The provider session is invalidated.
3. The shell terminates/reaps the fixture within shutdown_timeout_ms.
4. The failed request is not automatically replayed.
5. The next explicit user/request action starts a fresh provider session.
6. Ordinary completion and cooperative cancellation continue to reuse the persistent child and do not restart it.

No candidate-specific keep-versus-restart policy is permitted.

## 12. State retention

For benchmark equality:

- The response area holds only the current request plus a fixed fixture of four prior short messages.
- Starting a new ordinary request replaces the previous current-response body after completion/cancellation.
- Hiding the composer does not cancel the active request.
- Reopening the composer shows the current in-progress/completed response.
- Explicit cancel is the only normal user cancellation action.
- No database or unbounded transcript is used.

## 13. Fixture deliverables

The implementation task must create one shared provider under the benchmark area, not one provider per candidate language.

The frozen fixture package must include:

- manifest with all fields from section 1
- exact normal response payload
- text/visual fixture version
- reconstructed response SHA-256
- exact frame sequences
- exact physical-write fragmentation plan
- direct decoder-fragment vectors
- exact cancellation barrier
- exact exceptional recovery rules
- exact stderr fixture
- exact failure exit code
- exact frame limits
- exact timeout values
- provider executable identity and arguments
- provider working directory
- sanitized fixture-specific environment
- executable build/run instructions

Once frozen, any fixture change increments the fixture version and invalidates affected correctness/benchmark evidence across all candidates.
