# Agent Handoff — Common Fixture/Harness Freeze

Task ID: MASCOT-FIXTURE-001

Status: READY AFTER PLAN APPROVAL

## Objective

Prepare, validate, version, and freeze the common benchmark fixture/harness that will later be consumed unchanged by the Rust, Zig, and Go candidate implementations.

**Do not implement any candidate application code in rust/, zig/, or go/.**

The goal of this task is to remove candidate-specific interpretation from the benchmark before application implementation begins.

## Read first

Read these files in full before changing anything:

- README.md
- docs/PROTOTYPE_PLAN.md
- docs/BENCHMARK_PROTOCOL.md
- docs/ACCEPTANCE_MATRIX.md
- docs/TEXT_FIXTURES.md
- docs/MOCK_PROVIDER_CONTRACT.md
- rust/README.md
- zig/README.md
- go/README.md

The contracts above are normative. If implementation exposes a contradiction or impossible requirement, stop and report it rather than silently changing semantics.

## Required deliverables

Create shared experimental infrastructure under benchmark/ and assets/ only, plus documentation needed to freeze it.

Expected high-level shape:

    benchmark/
    ├── provider/
    ├── harness/
    ├── fixtures/
    ├── decoder-vectors/
    ├── manifest/
    └── README.md
    assets/
    └── mascot.<approved-format>

Exact internal names may differ if documented.

### 1. Shared mock provider

Implement one provider executable used unchanged by all three candidates.

It must satisfy docs/MOCK_PROVIDER_CONTRACT.md, including:

- persistent ordinary session
- NDJSON framing
- deterministic 100-chunk normal response
- deterministic payload hash
- canonical cancellation barrier after chunk 49
- provider-initiated client_request/client_response case
- stderr pressure case
- backpressure case
- maximum valid frame
- oversized-frame invalidation/recovery policy
- unexpected-exit case
- explicit shutdown
- shared timeouts
- deterministic terminal events
- emission timestamps at the contract-defined point

Do not create separate Rust/Zig/Go providers.

### 2. Direct decoder-fragment vectors

Create frozen byte-fragment vectors independent of OS pipe behavior.

Cover at minimum:

- one frame split across multiple fragments
- multiple frames in one fragment
- newline boundary split
- UTF-8 multi-byte sequence split
- maximum-size valid frame
- oversized frame marker/vector as appropriate

Document exact expected reconstructed frames or expected rejection.

### 3. Text/action fixture manifest

Materialize the exact shared values required by docs/TEXT_FIXTURES.md:

- F1-F10 logical strings
- benchmark font rule
- Windows keyboard layout for dead-key case
- exact dead-key sequence and expected output
- Microsoft Japanese IME romaji sequence
- expected committed Japanese text
- IME cancel action
- F1 selection keystrokes/caret endpoints
- F2 selection keystrokes/caret endpoints
- pre-approved native variations, if any
- visual evidence procedure

Do not broaden this into a general Unicode conformance suite.

### 4. Mascot asset freeze

Add the approved transparent mascot asset and freeze:

- file path
- SHA-256
- pixel dimensions
- logical benchmark dimensions
- alpha/hit-mask policy

If the currently available mascot asset has not been explicitly approved by the user, do **not** invent or silently substitute one. Leave the asset step BLOCKED and report the exact missing approval.

### 5. Fixture manifest

Create one versioned machine-readable manifest.

It must freeze every field required by docs/MOCK_PROVIDER_CONTRACT.md and docs/PROTOTYPE_PLAN.md, including:

- fixture version
- provider executable identity
- provider arguments
- working directory
- sanitized fixture-specific environment
- payload identities/hashes
- scenarios
- frame sizes
- timeouts
- cancellation behavior
- exceptional recovery
- fragmentation plan
- decoder vectors
- expected terminal events
- text fixture version
- mascot asset hash/dimensions
- UI logical dimensions needed by candidates

Choose JSON, YAML, or TOML deliberately and document why. The consumer contract must remain language-neutral.

### 6. Shared Windows benchmark-harness foundation

Implement the common harness pieces that do not require a candidate implementation yet:

- process launch wrapper
- QPC-compatible benchmark clock/timestamp representation
- provider launch with sanitized frozen environment
- raw result/event schema
- balanced candidate-order schedule representation
- process inventory/event schema
- resource-sampling schema
- output directory layout
- reproducibility metadata schema

Do not fabricate candidate metrics.

The actual visible-presentation/input-readiness observer may require a candidate window to qualify. Provide the hook/interface and document what remains to be qualified later.

### 7. Fixture validation

Add automated validation for the shared infrastructure itself.

At minimum validate:

- manifest schema/content
- response payload SHA-256
- normal provider frame sequence
- cancellation barrier
- child reuse after cancellation
- oversized-session teardown policy
- fresh session after oversized failure
- unexpected-exit scenario
- stderr pressure completes without provider-side deadlock
- direct decoder vectors are internally consistent
- sanitized provider environment is stable
- no secret/full inherited environment dump is persisted

Run and record all available checks.

## Implementation language for the shared harness

The fixture/harness language is **not** one of the candidate results.

Choose a pragmatic implementation that produces deterministic, portable tooling.

A single implementation is preferred.

If using Go for the shared provider/harness:

- pin the Go version
- sanitize GOGC/GOMEMLIMIT/GOMAXPROCS and relevant Go debug environment explicitly
- freeze those values in the manifest
- remember that provider resource use is excluded only because the same frozen provider is used for every candidate

If choosing another language, document the equivalent runtime/environment controls.

Do not use Electron, Chromium, WebView, Node, or a browser runtime.

## Scope boundaries

Forbidden in this task:

- Rust candidate application
- Zig candidate application
- Go candidate application
- real Codex integration
- Devin ACP
- terminal
- code viewer
- screenshots
- audio
- browser integration
- daemon/shell split
- RepoSuite
- cloud sync
- benchmark conclusions or language ranking

## Freeze procedure

Do not declare the fixture frozen merely because code exists.

Freeze only after:

1. all required shared infrastructure is present
2. validation passes, or any unavailable environment-only checks are explicitly listed
3. manifest is complete
4. exact fixture version is assigned
5. payload and asset hashes are recorded
6. working tree is clean
7. a dedicated freeze report is committed

Create:

    benchmark/FIXTURE_FREEZE.md

It must record:

- fixture version
- commit SHA
- toolchain versions
- provider identity
- manifest identity/hash
- mascot identity/hash, or explicit BLOCKED status if user approval is still required
- validation commands
- validation results
- known limitations
- what remains to qualify only after candidate apps exist

## Stop conditions

Stop and report without implementing around the issue if:

- two normative documents contradict each other
- the frozen provider semantics cannot be made deterministic
- a required manifest field is ambiguous
- implementing the common harness would require changing candidate-visible behavior
- the mascot asset is not approved
- a benchmark measurement would need candidate-specific semantics

## Final report

Return:

1. files changed
2. fixture version
3. implementation language/toolchain and why
4. validation commands and results
5. exact freeze commit SHA
6. manifest path/hash
7. provider binary/source identity
8. mascot asset path/hash or BLOCKED status
9. remaining observer qualification work
10. explicit statement:

    CANDIDATE_IMPLEMENTATION_READY

Only use that statement if the fixture is actually frozen and no unresolved blocker prevents Rust/Zig/Go application work.
