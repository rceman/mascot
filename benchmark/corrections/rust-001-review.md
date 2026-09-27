# Rust first-buildable implementation review

Status: **REJECTED FOR CORRECTNESS READINESS; preserve as the first complete implementation attempt, not an eligible benchmark candidate.**

The commit containing this record preserves the initial implemented Rust source before the focused review correction. Its parent design milestone is `a70199b86d943c7462e8cbc4f6c3a208fe00d539`. Candidate implementation order remains Rust, Zig, Go. These findings and the corrected common procedures must also be available to the later implementations; the resulting learning-order bias must be discussed, not concealed.

## Existing evidence

- `../results/windows/raw/rust-check-001/`: initial compiler diagnostics.
- `../results/windows/raw/rust-test-001/` through `rust-test-003/`: unit-test attempts; the final log records 19 passing tests.
- `../results/windows/raw/rust-build-001/` through `rust-build-003/`: release build attempts.
- `../results/windows/raw/rust-smoke-001/` through `rust-smoke-005/`: targeted smoke attempts. The last result is `PASS_SMOKE_ONLY`, with executable SHA-256 `0f7b26f6e6d079c7a445b86975d27a5e735006589edf800b12b4c92be2a5e12c`.

The narrow smoke does not certify provider scenarios, physical keyboard/IME behavior, visual rendering, DPI transitions, stability or performance. No candidate performance result is eligible at this point.

## Source-review findings requiring correction

1. `platform.rs::present_mascot` passes a pointer to an eight-byte `POINT` as the sixteen-byte output `RECT` required by `GetWindowRect`. This is an out-of-bounds native write. Do not run additional candidate measurements before correcting it.
2. Font creation divides by 72, treating 16 as points rather than the frozen 16 DIP. Both initial creation and relayout need DIP conversion. Composer placement also chooses a neighboring monitor from its proposed position while using mascot-monitor DPI; on a smaller neighboring work area the clamp bounds can invert. Placement and effective-DPI geometry must be tested at both scales.
3. Provider-to-UI delivery uses `try_push` and treats a full 64-frame queue as a protocol failure rather than applying backpressure. Conversely, UI-to-worker `send` uses a blocking push. The full-queue behavior violates the intended ownership and responsiveness rules.
4. Session generation fields are constructed but ignored by UI event dispatch. In particular, any failed terminal may overwrite the current request regardless of its ID. Worker-generated terminal/EOF races also require exactly-once terminal ownership.
5. The control reader uses unbounded `read_line` before checking frame size. Its shutdown cancels synchronous I/O only once, ignores the native wait result, then joins indefinitely; the reader closes the separate native thread handle while another thread may still use it. The input queue is closed too late to release a blocked producer.
6. Application shutdown closes telemetry and quits the UI before provider teardown has completed. The provider stop flag can interrupt the graceful shutdown path, while clearing protocol state and stopping reader progress can hide missing acknowledgements. Native waits should use the owned Child handle, enforce one total deadline, and never fall through to an unbounded wait after a failed timed wait.
7. RefCell borrows are held across re-entrant native calls in composer show/hide, response refresh, snapshots, relayout and destruction. Native callback and model ownership must match the architecture's short-borrow rule. OLE initialization must be balanced, and partial resource-creation failures need correct disposal.
8. The response is rewritten in full for every chunk rather than appended/coalesced through the planned native path. Response-bound overflow silently truncates text while still acknowledging the sequence; overflow must fail explicitly instead. Accepted timestamps should follow model acceptance.
9. Paint counting covers only the parent composer, missing native child-control paints. Cache counters label a retained CPU vector as a DIB although the native bitmap is deleted after presentation. Report actual resources and disclose unobservable native text-cache/caret behavior.
10. Ctrl+Enter interception occurs after the message loop has called TranslateMessage, which can queue a newline even when the subsequent keydown handler consumes the shortcut. Intercept the non-composing shortcut before translation; physical key/IME verification remains required.
11. The mascot reports caption hit testing but handles only client right-button messages for its Exit menu. Handle the corresponding non-client/context-menu path. Thread-only work messages may also be lost inside native modal move/menu loops; route work through the owned HWND.
12. Ordinary GUI mode starts an output writer even without control transport; an unusable stdout can leave telemetry undrained and affect later requests. Manual GUI behavior must not depend on a benchmark stdout consumer.
13. The implementation introduced a bespoke SHA-256 solely for asset integrity and experienced a padding-loop hang. Replace that unnecessary owned cryptographic implementation with the established Windows SHA-256 API, retaining digest regression checks. This is a focused native-boundary correction, not a language-ranking optimization.
14. The SDK build script silently searches arbitrary installed SDK versions if the pinned resource compiler is absent. Fail clearly instead of silently changing the build configuration.

These are source-review findings. Where a runtime consequence has not yet been reproduced, it must not be represented as a completed acceptance-test failure. A passing compile/smoke did not prove absence of these defects.

## Shared smoke-checker correction

The first version of `verify_candidate_smoke.py` does not establish a per-monitor-v2 caller context before native geometry queries. Its geometry check is therefore not qualified across display scales. The later passing smoke runs used the 100% virtual monitor; that workaround is not sufficient evidence for the 200% configuration. Correct the common checker, version its result schema, record effective DPI/geometry/font observations, and rerun the same corrected procedure at both scales. Preserve the old results unchanged.

## Next gate

Apply one consolidated focused correction with regression tests for bounds, queue pressure, terminal ownership and shutdown. Preserve the first source snapshot and all raw failures. Rerun targeted checks only; full shared correctness and the qualified Windows benchmark remain pending for all candidates. Do not derive comparative results or a cross-platform winner from this milestone.
