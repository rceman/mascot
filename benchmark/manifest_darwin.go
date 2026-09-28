//go:build darwin

package main

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
)

// freezeManifest writes manifest/fixture.darwin-arm64.json once per frozen
// macOS fixture version. It preserves every payload/protocol identity from
// windows-v1.0.2 while recording the native darwin/arm64 provider binary,
// launch paths and sanitized environment. It refuses to overwrite an existing
// manifest: a semantic or binary change requires a deliberate version bump in
// platform_darwin.go, matching the post-freeze versioning rule.
func freezeManifest(root string) error {
	output := filepath.Join(root, "manifest", manifestFile)
	if _, err := os.Stat(output); err == nil {
		return errors.New("macOS manifest already frozen; bump fixtureVersion deliberately")
	}
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	identities := make(map[string]string)
	for _, path := range []string{"fixtures/text.json", "fixtures/chunks.json", "fixtures/response.txt", "fixtures/history.json", "decoder-vectors/vectors.json", "../assets/mascot.png", "harness/control-v1.json", "requirements-source.txt"} {
		digest, err := hashFile(filepath.Join(root, filepath.FromSlash(path)))
		if err != nil {
			return err
		}
		identities[path] = digest
	}
	binaryHash, err := hashFile(executable)
	if err != nil {
		return err
	}
	manifest := map[string]any{
		"schema": "mascot-fixture-1", "version": fixtureVersion, "base_fixture": "windows-v1.0.2", "files_sha256": identities,
		"provider": map[string]any{"path": filepath.Join(root, "bin", "fixture"), "sha256": binaryHash, "arguments": []string{"provider", root}, "cwd": root, "environment": providerEnvironment(), "inherit_environment": false, "implementation": runtime.Version(), "build": "CGO_ENABLED=1 go build -trimpath -buildvcs=false -ldflags=\"-s -w -buildid=\" -o bin/fixture ."},
		"asset":    map[string]any{"status": "APPROVED", "path": "../assets/mascot.png", "pixel_width": 1254, "pixel_height": 1254, "logical_width_dip": 64, "logical_height_dip": 64, "hit_mask": "source alpha > 0; no expansion; hit coordinates mapped to source texels at current effective scale factor", "sha256": identities["../assets/mascot.png"]},
		"ui":       map[string]any{"composer_client_width_dip": 640, "composer_client_height_dip": 480, "input_height_dip": 128, "response_height_dip": 272, "margin_dip": 12, "hotkey": "CTRL+ALT+SPACE", "submit": "CTRL+ENTER", "cancel": "CTRL+ALT+ESCAPE", "always_on_top": "mascot above ordinary windows, no activation merely by showing; composer activates on explicit hotkey", "input_limit_utf16_units": 4096, "response_limit_utf8_bytes": 262144, "history_messages": 4, "hide_cancels_request": false},
		"protocol": map[string]any{"transport": "UTF-8 NDJSON", "max_stdin_frame_bytes_including_lf": frameLimit, "max_stdout_frame_bytes_including_lf": frameLimit, "oversized_frame_bytes_including_lf": frameLimit + 1, "request_extension": "optional scenario string, default normal; same persistent session accepts different scenarios", "normal_chunks": 100, "normal_interval_ms": 10, "chunk_timestamp": "emit_qpc is a zero-padded 20-character decimal mach_absolute_time count captured immediately before the first physical write containing bytes of the logical frame; fixed-width stamp copied into already serialized frame; start carries qpc_frequency (mach ticks per second)", "receipt_timestamp": "mach_absolute_time when LF completing a logical frame is received, before JSON decoding", "shutdown_timeout_ms": 2000, "cancel_timeout_ms": 1000, "client_response_timeout_ms": 1000, "invalid_protocol_exit_code": 64, "unexpected_exit_code": 23, "normal_exit_code": 0},
		"scenarios": map[string]any{
			"normal":          map[string]any{"sequence": "start, chunk(seq0..99), complete", "payload": "fixtures/chunks.json", "write_plan": "whole frames except seq4 split after first byte of first multibyte UTF-8 scalar"},
			"fragmented":      map[string]any{"sequence": "start, chunk(seq0..99), complete", "write_plan": "seq%10=1 cuts at byte offsets1,7,end; seq%10=2 and next seq3 coalesced in one write; seq%10=4 split after first byte of first multibyte UTF-8 scalar; seq%10=5 split before LF; otherwise whole. Both coalesced frames share first-write mach stamp; scheduling jitter reported separately."},
			"cancel":          map[string]any{"sequence": "start, chunk(seq0..49), barrier, client cancel, cancelled(last_seq49)", "forbidden": []string{"chunk50", "complete"}, "reuse_child": true},
			"client_request":  map[string]any{"sequence": "start, client_request(request_id1, benchmark.confirm, value ok), wait for accepted true response, chunk(seq0..99), complete"},
			"stderr":          map[string]any{"bytes": 262144, "content": "fixture-stderr followed by LF, repeated and truncated to262144bytes", "stdout": "normal", "parallel_stderr": true},
			"backpressure":    map[string]any{"chunks": 256, "interval_ms": 0, "text": "chunks.json[seq%100]", "headline_latency": false},
			"maximum":         map[string]any{"sequence": "start(chunks1), chunk(seq0), complete", "bytes_including_lf": 65536, "text": "ASCII x padding to exact serialized frame size"},
			"oversized":       map[string]any{"sequence": "start(chunks1), invalid oversized chunk", "bytes_including_lf": 65537, "recovery": "fail current request; invalidate session; terminate/reap within2000ms; no automatic replay; next explicit request starts fresh child"},
			"unexpected_exit": map[string]any{"sequence": "start, chunk(seq0..6), process exit23", "recovery": "provider-failed state; next explicit request starts fresh child"},
		},
		"decoder_vectors": "decoder-vectors/vectors.json", "text_fixture": "fixtures/text.json", "candidate_control": "harness/control-v1.json", "source_dependency_lock": "requirements-source.txt",
		"schedule":                 map[string]any{"order_rule": "macOS Stage B validates rust then go; not the full balanced campaign", "fresh_launches_each": 10, "first_activation_lifetimes_each": 5, "warm_activations_per_lifetime": []int{10}, "steady_state_independent_runs": 3, "steady_settle_ms": 60000, "steady_collection_ms": 30000, "active_sample_ms": 100, "focused_idle_ms": 600000, "stability_batches": 3, "stability_open_close_ops": 100, "stability_submit_ops": 50, "stability_sample_every": 10, "stability_content_modes": []string{"fixed", "bounded_varying"}},
		"observer":                 map[string]any{"state": "INTERFACE_ONLY_PENDING_QUALIFICATION", "clock": "shared mach_absolute_time ticks/frequency; no language-specific epoch conversion", "hotkey_origin": "external mach_absolute_time immediately before CGEvent post", "visible_end": "externally observed compositor output via CGWindowList on-screen window appearance; screen capture for pixel evidence", "input_ready": "separate injected text probe with exact readback", "stream": "correlate first externally visible content with accepted sequence; coalesced sequences may share a presentation", "qualification_required_before_headline": true, "resolution_and_uncertainty_required": true},
		"source_efficiency":        map[string]any{"tokenizer": "tiktoken/o200k_base", "package_version": "0.12.0", "normalization": "UTF-8 committed bytes with LF line endings; no minification or source rewriting for counts", "model_token_usage": "not exposed reliably; do not fabricate"},
		"process_inventory_fields": []string{"pid", "parent_pid", "role", "creation_qpc", "exit_qpc", "counted", "exclusion_reason"},
		"resource_sample_fields":   []string{"qpc", "pid", "private_working_set_bytes", "private_commit_bytes", "working_set_bytes", "user_cpu_100ns", "kernel_cpu_100ns", "threads", "kernel_handles", "user_objects", "gdi_objects", "live_windows", "live_children", "redraws", "presents", "bounded_queue_depth"},
		"raw_event_fields":         []string{"run_id", "fixture_version", "candidate", "candidate_build_sha256", "configuration_id", "process_lifetime", "scenario", "qpc", "qpc_frequency", "kind", "request_id", "seq", "details"},
		"output":                   "results/macos/raw/<immutable-run-id>/; per-process rows and simultaneous aggregate samples; null plus reason for unavailable fields, never invented zeros",
	}
	if err := putJSON(output, manifest); err != nil {
		return err
	}
	return nil
}
