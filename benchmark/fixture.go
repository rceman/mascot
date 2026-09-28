package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

func hashFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	digest := sha256.Sum256(data)
	return hex.EncodeToString(digest[:]), nil
}

func putJSON(path string, value any) error {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, append(data, '\n'), 0644)
}

func reportEnvironment() error {
	values := make(map[string]string)
	for key := range providerEnvironment() {
		value, exists := os.LookupEnv(key)
		if !exists {
			return fmt.Errorf("required fixture environment absent: %s", key)
		}
		values[key] = value
	}
	return json.NewEncoder(os.Stdout).Encode(map[string]any{"values": values, "gomaxprocs": runtime.GOMAXPROCS(0)})
}

type vector struct {
	Name      string   `json:"name"`
	Fragments [][]byte `json:"fragments_base64"`
	Frames    [][]byte `json:"expected_frames_base64"`
	Reject    bool     `json:"reject"`
}

func makeVectors() []vector {
	first := []byte("{\"type\":\"chunk\",\"id\":1,\"seq\":0,\"text\":\"Ā\"}\n")
	second := []byte("{\"type\":\"complete\",\"id\":1}\n")
	at := strings.Index(string(first), "Ā") + 1
	max := chunk(1, 0, strings.Repeat("x", frameLimit-len(chunk(1, 0, ""))))
	over := chunk(1, 0, strings.Repeat("x", frameLimit+1-len(chunk(1, 0, ""))))
	return []vector{
		{"small_fragments", [][]byte{first[:1], first[1:7], first[7:]}, [][]byte{first}, false},
		{"coalesced_frames", [][]byte{append(append([]byte{}, first...), second...)}, [][]byte{first, second}, false},
		{"newline_boundary", [][]byte{first[:len(first)-1], first[len(first)-1:], second}, [][]byte{first, second}, false},
		{"utf8_boundary", [][]byte{first[:at], first[at:]}, [][]byte{first}, false},
		{"maximum_valid", [][]byte{max[:32768], max[32768:65535], max[65535:]}, [][]byte{max}, false},
		{"oversized", [][]byte{over[:32768], over[32768:65536], over[65536:]}, nil, true},
	}
}

func drawMascot(path string) error {
	img := image.NewNRGBA(image.Rect(0, 0, 128, 128))
	inside := func(x, y, cx, cy, radius float64) bool { return (x-cx)*(x-cx)+(y-cy)*(y-cy) < radius*radius }
	for y := 0; y < 128; y++ {
		for x := 0; x < 128; x++ {
			coverage := 0
			for sy := 0; sy < 4; sy++ {
				for sx := 0; sx < 4; sx++ {
					px, py := float64(x)+float64(sx)/4, float64(y)+float64(sy)/4
					if inside(px, py, 64, 69, 43) || inside(px, py, 33, 32, 18) || inside(px, py, 95, 32, 18) {
						coverage++
					}
				}
			}
			if coverage == 0 {
				continue
			}
			pixel := color.NRGBA{R: 35, G: 160, B: 175, A: uint8(coverage * 255 / 16)}
			if inside(float64(x), float64(y), 49, 62, 5) || inside(float64(x), float64(y), 79, 62, 5) || inside(float64(x), float64(y), 64, 81, 4) {
				pixel.R, pixel.G, pixel.B = 20, 30, 35
			}
			img.SetNRGBA(x, y, pixel)
		}
	}
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, img); err != nil {
		return err
	}
	if existing, err := os.ReadFile(path); err == nil {
		if bytes.Equal(existing, encoded.Bytes()) {
			return nil
		}
		return errors.New("existing mascot differs; refusing to overwrite it")
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
	if err != nil {
		return err
	}
	if _, err := file.Write(encoded.Bytes()); err != nil {
		file.Close()
		return err
	}
	return file.Close()
}

func materialize(root string, refresh bool) error {
	if _, err := os.Stat(filepath.Join(root, "FIXTURE_FREEZE.md")); err == nil {
		return errors.New("fixture freeze exists; no automatic regeneration permitted")
	}
	if previous, err := os.ReadFile(filepath.Join(root, "manifest", "fixture.json")); err == nil {
		if !refresh {
			return errors.New("fixture already materialized; version deliberately before changing it")
		}
		archive := filepath.Join(root, "results", "windows", "raw", "unfrozen-manifest-"+time.Now().UTC().Format("20060102T150405.000000000Z")+".json")
		if err := os.WriteFile(archive, previous, 0644); err != nil {
			return err
		}
	}
	for _, directory := range []string{"fixtures", "decoder-vectors", "manifest", "results/windows/raw", "corrections", "../assets"} {
		if err := os.MkdirAll(filepath.Join(root, filepath.FromSlash(directory)), 0755); err != nil {
			return err
		}
	}
	text := map[string]any{
		"version": "text-v1.0.0",
		"F1":      "Ae\u0301B", "F2": "A\U0001f468\u200d\U0001f4bbB", "F3": "English العربية English",
		"F4":                          []string{"Ārā līst, bet mēs turpinām darbu.", "Проверка отображения текста.", "\U0001f468\u200d\U0001f4bb \U0001f9d1\U0001f3fd\u200d\U0001f680 \u2764\ufe0f\u200d\U0001f525"},
		"F5":                          map[string]any{"layout": "US-International", "klid": "00020409", "keys": []string{"APOSTROPHE", "E"}, "expected": "é", "requests_during_composition": 0},
		"F6":                          map[string]any{"ime": "Microsoft Japanese IME", "langid": 1041, "service_clsid": "03B5835F-F03C-411B-9CE2-AA23E1171E36", "profile_guid": "A76C93D9-5523-4E90-AAFA-4DB112F9AC76", "romaji": "nihonn", "commit": []string{"ENTER"}, "expected": "にほん", "cancel": []string{"ESCAPE", "ESCAPE"}, "cancel_prior": "baseline"},
		"F7":                          map[string]any{"submit": "CTRL+ENTER", "during_composition_requests": 0, "after_committed_explicit_submit_requests": 1},
		"F8":                          map[string]any{"hide": "GLOBAL_HOTKEY", "policy": "cancel composition before hiding, restore last committed text, no request"},
		"F9":                          []string{"F1", "F2", "F3", "F4"},
		"F10":                         "first line\nĀrā līst\nПроверка\n\U0001f468\u200d\U0001f4bb",
		"logical_line_break":          "LF; Windows clipboard CRLF converts to logical LF without changing line structure",
		"font":                        map[string]any{"family": "Segoe UI", "size_dip": 16, "fallback": "Windows system fallback, including Segoe UI Emoji and installed Japanese fonts; no candidate-private font"},
		"F1_selection":                map[string]any{"keys": []string{"CTRL+END", "LEFT", "LEFT", "SHIFT+RIGHT", "CTRL+C"}, "utf16_selection_start": 1, "utf16_selection_end": 3, "copied": "e\u0301", "after_selected_delete": "AB"},
		"F2_selection":                map[string]any{"keys": []string{"CTRL+END", "LEFT", "LEFT", "SHIFT+RIGHT", "CTRL+C"}, "utf16_selection_start": 1, "utf16_selection_end": 6, "copied": "\U0001f468\u200d\U0001f4bb", "after_selected_delete": "AB"},
		"permitted_native_variations": []string{},
		"visual_evidence":             "Capture only candidate window with an external screen capture after each action; record exact logical UTF-8, UTF-16 caret endpoints, composition state and request count. Inspect images for attached marks, joined emoji, Arabic contextual shaping and bidi. Composer and response get separate evidence. Never infer visual PASS from bytes or APIs.",
	}
	if err := putJSON(filepath.Join(root, "fixtures", "text.json"), text); err != nil {
		return err
	}
	lines := []string{"Ārā līst, bet mēs turpinām darbu.", "Проверка отображения текста.", "Ae\u0301B A\U0001f468\u200d\U0001f4bbB \U0001f9d1\U0001f3fd\u200d\U0001f680 \u2764\ufe0f\u200d\U0001f525", "English العربية English"}
	chunks := make([]string, 100)
	for index := range chunks {
		chunks[index] = fmt.Sprintf("%03d|Ā %s\n", index, lines[index%len(lines)])
	}
	if err := putJSON(filepath.Join(root, "fixtures", "chunks.json"), chunks); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(root, "fixtures", "response.txt"), []byte(strings.Join(chunks, "")), 0644); err != nil {
		return err
	}
	if err := putJSON(filepath.Join(root, "fixtures", "history.json"), []string{"User: Hello.", "Assistant: Ready.", "User: Show the benchmark.", "Assistant: Using the frozen fixture."}); err != nil {
		return err
	}
	if err := putJSON(filepath.Join(root, "decoder-vectors", "vectors.json"), makeVectors()); err != nil {
		return err
	}
	asset := filepath.Join(root, "..", "assets", "mascot.png")
	// The approved mascot asset is user-provided; it must already exist.
	if _, err := os.Stat(asset); err != nil {
		return err
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
		"schema": "mascot-fixture-1", "version": fixtureVersion, "base_sha": "2f55ae825848e183a7840032782e37cbc54e641d", "files_sha256": identities,
		"provider": map[string]any{"path": executable, "sha256": binaryHash, "arguments": []string{"provider", root}, "cwd": root, "environment": providerEnvironment(), "inherit_environment": false, "implementation": runtime.Version(), "build": "go build -trimpath -buildvcs=false -ldflags=\"-s -w -buildid=\" -o bin/fixture.exe ."},
		"asset":    map[string]any{"status": "APPROVED", "path": "../assets/mascot.png", "pixel_width": 1254, "pixel_height": 1254, "logical_width_dip": 64, "logical_height_dip": 64, "hit_mask": "source alpha > 0; no expansion; hit coordinates mapped to source texels at current effective DPI", "sha256": identities["../assets/mascot.png"]},
		"ui":       map[string]any{"composer_client_width_dip": 640, "composer_client_height_dip": 480, "input_height_dip": 128, "response_height_dip": 272, "margin_dip": 12, "hotkey": "CTRL+ALT+SPACE", "submit": "CTRL+ENTER", "cancel": "CTRL+ALT+ESCAPE", "always_on_top": "mascot above ordinary windows, no activation merely by showing; composer activates on explicit hotkey", "input_limit_utf16_units": 4096, "response_limit_utf8_bytes": 262144, "history_messages": 4, "hide_cancels_request": false},
		"protocol": map[string]any{"transport": "UTF-8 NDJSON", "max_stdin_frame_bytes_including_lf": frameLimit, "max_stdout_frame_bytes_including_lf": frameLimit, "oversized_frame_bytes_including_lf": frameLimit + 1, "request_extension": "optional scenario string, default normal; same persistent session accepts different scenarios", "normal_chunks": 100, "normal_interval_ms": 10, "chunk_timestamp": "emit_qpc is a zero-padded 20-character decimal QPC count captured immediately before the first physical Write containing bytes of the logical frame; fixed-width stamp copied into already serialized frame; start carries qpc_frequency", "receipt_timestamp": "QPC when LF completing a logical frame is received, before JSON decoding", "shutdown_timeout_ms": 2000, "cancel_timeout_ms": 1000, "client_response_timeout_ms": 1000, "invalid_protocol_exit_code": 64, "unexpected_exit_code": 23, "normal_exit_code": 0},
		"scenarios": map[string]any{
			"normal":          map[string]any{"sequence": "start, chunk(seq0..99), complete", "payload": "fixtures/chunks.json", "write_plan": "whole frames except seq4 split after first byte of first multibyte UTF-8 scalar"},
			"fragmented":      map[string]any{"sequence": "start, chunk(seq0..99), complete", "write_plan": "seq%10=1 cuts at byte offsets1,7,end; seq%10=2 and next seq3 coalesced in one write; seq%10=4 split after first byte of first multibyte UTF-8 scalar; seq%10=5 split before LF; otherwise whole. Both coalesced frames share first-write QPC; scheduling jitter reported separately."},
			"cancel":          map[string]any{"sequence": "start, chunk(seq0..49), barrier, client cancel, cancelled(last_seq49)", "forbidden": []string{"chunk50", "complete"}, "reuse_child": true},
			"client_request":  map[string]any{"sequence": "start, client_request(request_id1, benchmark.confirm, value ok), wait for accepted true response, chunk(seq0..99), complete"},
			"stderr":          map[string]any{"bytes": 262144, "content": "fixture-stderr followed by LF, repeated and truncated to262144bytes", "stdout": "normal", "parallel_stderr": true},
			"backpressure":    map[string]any{"chunks": 256, "interval_ms": 0, "text": "chunks.json[seq%100]", "headline_latency": false},
			"maximum":         map[string]any{"sequence": "start(chunks1), chunk(seq0), complete", "bytes_including_lf": 65536, "text": "ASCII x padding to exact serialized frame size"},
			"oversized":       map[string]any{"sequence": "start(chunks1), invalid oversized chunk", "bytes_including_lf": 65537, "recovery": "fail current request; invalidate session; terminate/reap within2000ms; no automatic replay; next explicit request starts fresh child"},
			"unexpected_exit": map[string]any{"sequence": "start, chunk(seq0..6), process exit23", "recovery": "provider-failed state; next explicit request starts fresh child"},
		},
		"decoder_vectors": "decoder-vectors/vectors.json", "text_fixture": "fixtures/text.json", "candidate_control": "harness/control-v1.json", "source_dependency_lock": "requirements-source.txt",
		"schedule":                 map[string]any{"balanced_blocks": [][]string{{"rust", "zig", "go"}, {"zig", "go", "rust"}, {"go", "rust", "zig"}, {"go", "zig", "rust"}, {"zig", "rust", "go"}, {"rust", "go", "zig"}}, "fresh_launches_each": 30, "post_reboot_first_launches_each": 3, "order_rule": "For each scenario, repetition r uses balanced_blocks[r modulo 6], executing candidates sequentially in that order; never interleave candidate processes. Cold-after-boot order must be recorded explicitly, with first position balanced across boots.", "first_activation_lifetimes_each": 10, "warm_activations_per_lifetime": []int{34, 33, 33}, "steady_state_independent_runs": 3, "steady_settle_ms": 60000, "steady_collection_ms": 30000, "active_sample_ms": 100, "focused_idle_ms": 600000, "stability_batches": 3, "operations_per_batch": 100, "stability_sample_every": 10, "stability_content_modes": []string{"fixed", "bounded_varying"}, "varying_prompt": "cycle 256 IDs/content keys in the fixed input bound; no candidate cache may claim eviction coverage without evidence its declared cap is exercised"},
		"observer":                 map[string]any{"state": "INTERFACE_ONLY_PENDING_QUALIFICATION", "clock": "shared Windows QPC counter/frequency; no language-specific epoch conversion", "hotkey_origin": "external QPC immediately before common SendInput call", "visible_end": "externally observed screen/compositor output, not app callback or presentation API return", "input_ready": "separate injected text probe with exact readback", "stream": "correlate first externally visible content with accepted sequence; coalesced sequences may share a presentation", "qualification_required_before_headline": true, "resolution_and_uncertainty_required": true},
		"source_efficiency":        map[string]any{"tokenizer": "tiktoken/o200k_base", "package_version": "0.12.0", "normalization": "UTF-8 committed bytes with LF line endings; no minification or source rewriting for counts", "model_token_usage": "not exposed reliably; do not fabricate"},
		"process_inventory_fields": []string{"pid", "parent_pid", "role", "creation_qpc", "exit_qpc", "counted", "exclusion_reason"},
		"resource_sample_fields":   []string{"qpc", "pid", "private_working_set_bytes", "private_commit_bytes", "working_set_bytes", "user_cpu_100ns", "kernel_cpu_100ns", "threads", "kernel_handles", "user_objects", "gdi_objects", "live_windows", "live_children", "redraws", "presents", "bounded_queue_depth"},
		"raw_event_fields":         []string{"run_id", "fixture_version", "candidate", "candidate_build_sha256", "configuration_id", "process_lifetime", "scenario", "qpc", "qpc_frequency", "kind", "request_id", "seq", "details"},
		"output":                   "results/windows/raw/<immutable-run-id>/; per-process rows and simultaneous aggregate samples; null plus reason for unavailable fields, never invented zeros",
	}
	return putJSON(filepath.Join(root, "manifest", "fixture.json"), manifest)
}
