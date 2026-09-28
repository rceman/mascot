package main

// macOS UI layer: same ownership model as the Windows candidate. The locked
// main goroutine runs the NSApplication run loop; all UI-owned state is
// touched only on that thread (either inside AppKit callbacks or inside the
// dispatch_async_f pumps that mirror the WM_APP_* posts).

import (
	"encoding/json"
	"errors"
	"fmt"
	"image"
	"image/png"
	"os"
	"strconv"
	"strings"
	"time"
	"unicode/utf16"
	"unsafe"
)

type model struct {
	providerState  string
	requestID      uint64
	requestCount   uint64
	lastSeq        int64
	response       string
	providerPID    uint32
	scenario       string
	runInvalid     string
	hasRunInvalid  bool
	providerError  string
	hasProviderErr bool
	generation     uint64
}

type composer struct {
	window   unsafe.Pointer
	input    unsafe.Pointer
	response unsafe.Pointer
	status   unsafe.Pointer
	send     unsafe.Pointer
	cancel   unsafe.Pointer
}

type UI struct {
	cfg                     *config
	mascotPtr               unsafe.Pointer
	mascotSource            []byte // straight (non-premultiplied) RGBA
	mascotSrcW              int
	mascotSrcH              int
	composer                *composer
	m                       model
	composing               bool
	snapshot                *textSnapshot
	uiEvents                *boundedQueue[providerEvent]
	commands                *boundedQueue[map[string]json.RawMessage]
	records                 *boundedQueue[string]
	provider                *provider
	controlEnabled          bool
	shutdownStarted         bool
	cancelPending           bool
	pendingShutdownToken    json.RawMessage
	hasPendingShutdownToken bool
	presents                uint64
	paints                  uint64
}

func eventMatches(generation, requestID, modelGeneration, modelRequestID uint64) bool {
	return generation == modelGeneration && requestID == modelRequestID
}

func appendBounded(response *string, text string, limit int) error {
	if strings.IndexByte(text, 0) >= 0 || len(*response)+len(text) > limit {
		return errors.New("response text exceeds native buffer contract")
	}
	*response += text
	return nil
}

func (ui *UI) record(value any) {
	if !ui.controlEnabled || ui.records == nil {
		return
	}
	line, err := json.Marshal(value)
	if err != nil {
		return
	}
	if !ui.records.tryPush(string(line)) {
		ui.m.runInvalid = "control output queue overflow"
		ui.m.hasRunInvalid = true
		fmt.Fprintln(os.Stderr, "control output queue overflow; run invalid")
	}
}

// decodePNG decodes the asset to straight (non-premultiplied) RGBA plus its
// pixel dimensions. NSBitmapImageRep is created with bitmapFormat=0, which
// treats the data as non-premultiplied.
func decodePNG(path string) ([]byte, int, int, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, 0, 0, fmt.Errorf("open asset: %w", err)
	}
	defer f.Close()
	img, err := png.Decode(f)
	if err != nil {
		return nil, 0, 0, fmt.Errorf("png decode: %w", err)
	}
	w, h := img.Bounds().Dx(), img.Bounds().Dy()
	if w <= 0 || h <= 0 {
		return nil, 0, 0, errors.New("asset has empty extent")
	}
	nrgba, ok := img.(*image.NRGBA)
	if !ok {
		converted := image.NewNRGBA(image.Rect(0, 0, w, h))
		for y := 0; y < h; y++ {
			for x := 0; x < w; x++ {
				converted.Set(x, y, img.At(x, y))
			}
		}
		nrgba = converted
	}
	return nrgba.Pix, w, h, nil
}

func (ui *UI) ensureComposer() error {
	if ui.composer != nil {
		return nil
	}
	uiConf := ui.cfg.manifest.UI
	handles := mascotCreateComposer(
		ui.mascotPtr,
		float64(uiConf.ComposerClientWidthDip),
		float64(uiConf.ComposerClientHeightDip),
		float64(uiConf.InputHeightDip),
		float64(uiConf.ResponseHeightDip),
		float64(uiConf.MarginDip),
	)
	if handles.window == nil {
		return errors.New("composer creation failed")
	}
	ui.composer = &composer{
		window:   handles.window,
		input:    handles.input,
		response: handles.response,
		status:   handles.status,
		send:     handles.send,
		cancel:   handles.cancel,
	}
	return nil
}

func (ui *UI) showComposer() error {
	if err := ui.ensureComposer(); err != nil {
		return err
	}
	mascotShowComposer(ui.composer)
	return nil
}

func (ui *UI) composerVisible() bool {
	return ui.composer != nil && mascotComposerVisible(ui.composer)
}

func (ui *UI) hideComposer() {
	if ui.composer == nil {
		return
	}
	if ui.composing || mascotHasMarked(ui.composer.input) {
		mascotUnmark(ui.composer.input)
		if snap := ui.snapshot; snap != nil {
			mascotTextSet(ui.composer.input, snap.text)
			mascotSelectionSet(ui.composer.input, snap.selStart, snap.selEnd-snap.selStart)
		}
		ui.composing = false
	}
	mascotHideComposer(ui.composer)
}

func (ui *UI) submit() error {
	if ui.shutdownStarted {
		return errors.New("shutdown in progress")
	}
	if ui.m.providerState == "streaming" {
		return errors.New("a request is already active")
	}
	if ui.composing {
		return errors.New("submit disabled while composing")
	}
	if ui.composer == nil {
		return errors.New("composer not created")
	}
	prompt := mascotTextGet(ui.composer.input)
	m := &ui.m
	id := m.requestCount + 1
	scenario := m.scenario
	m.scenario = "normal"
	m.requestCount = id
	m.requestID = id
	m.lastSeq = -1
	m.response = ""
	m.providerError = ""
	m.hasProviderErr = false
	m.providerState = "streaming"
	ui.cancelPending = false
	if ui.provider != nil {
		if err := ui.provider.sendRequest(id, prompt, scenario); err != nil {
			m.providerState = "failed"
			m.runInvalid = err.Error()
			m.hasRunInvalid = true
			return err
		}
	}
	ui.resetResponseView()
	ui.refreshStatus()
	return nil
}

func (ui *UI) cancelRequest() error {
	if ui.m.providerState != "streaming" || ui.m.requestID == 0 || ui.cancelPending {
		return nil
	}
	id := ui.m.requestID
	if ui.provider == nil {
		return nil
	}
	if err := ui.provider.sendCancel(id); err != nil {
		ui.m.runInvalid = "cancel enqueue failed: " + err.Error()
		ui.m.hasRunInvalid = true
		return err
	}
	ui.cancelPending = true
	return nil
}

func (ui *UI) resetResponseView() {
	if ui.composer != nil {
		mascotTextSet(ui.composer.response, ui.cfg.historyPrefix)
	}
}

func (ui *UI) appendResponseView(text string) {
	if ui.composer != nil {
		mascotTextAppend(ui.composer.response, text)
	}
}

func (ui *UI) refreshStatus() {
	if ui.composer != nil {
		mascotStatusSet(ui.composer.status, ui.m.providerState)
	}
}

func (ui *UI) dispatchProviderEvents() {
	for i := 0; i < 64; i++ {
		ev, ok := ui.uiEvents.pop()
		if !ok {
			break
		}
		switch ev.kind {
		case evStarted:
			if ev.generation >= ui.m.generation {
				ui.m.generation = ev.generation
				ui.m.providerPID = ev.pid
			}
			ui.refreshStatus()
		case evChunk:
			accepted := int64(0)
			acceptedOK := false
			invalid := false
			if eventMatches(ev.generation, ev.id, ui.m.generation, ui.m.requestID) {
				limit := int(ui.cfg.manifest.UI.ResponseLimitUTF8Bytes)
				if err := appendBounded(&ui.m.response, ev.text, limit); err == nil {
					ui.m.lastSeq = int64(ev.seq)
					accepted = qpc()
					acceptedOK = true
				} else {
					ui.m.runInvalid = err.Error()
					ui.m.hasRunInvalid = true
					invalid = true
				}
			}
			if acceptedOK {
				ui.record(map[string]any{
					"event":        "chunk_accepted",
					"request_id":   ev.id,
					"seq":          ev.seq,
					"accepted_qpc": strconv.FormatInt(accepted, 10),
				})
				ui.appendResponseView(ev.text)
			} else if invalid {
				ui.cancelRequest()
			}
		case evTerminal:
			matched := false
			if eventMatches(ev.generation, ev.id, ui.m.generation, ui.m.requestID) {
				ui.m.providerState = ev.termKind
				ui.m.lastSeq = ev.lastSeq
				ui.cancelPending = false
				matched = true
			}
			if matched {
				ui.record(map[string]any{
					"event":      "terminal",
					"request_id": ev.id,
					"kind":       ev.termKind,
					"last_seq":   ev.lastSeq,
					"qpc":        strconv.FormatInt(qpc(), 10),
				})
			}
			ui.refreshStatus()
		case evSessionClosed:
			if ev.generation == ui.m.generation {
				ui.m.providerPID = 0
				if ev.hasErr {
					ui.m.providerState = "failed"
					ui.m.providerError = ev.err
					ui.m.hasProviderErr = true
				}
			}
			ui.refreshStatus()
		case evStopped:
			if ui.hasPendingShutdownToken {
				token := ui.pendingShutdownToken
				ui.hasPendingShutdownToken = false
				ui.pendingShutdownToken = nil
				if ev.hasErr {
					ui.record(map[string]any{"token": token, "ok": false, "error": ev.err})
				} else {
					ui.record(map[string]any{"token": token, "ok": true})
				}
			}
			ui.commands.close()
			if ui.records != nil {
				ui.records.close()
			}
			mascotTerminate()
		}
	}
}

func (ui *UI) stateJSON() map[string]any {
	visible := ui.composerVisible()
	ptrStr := func(p unsafe.Pointer) string {
		if p == nil {
			return "0"
		}
		return strconv.FormatUint(uint64(uintptr(p)), 10)
	}
	var window, input, response unsafe.Pointer
	if ui.composer != nil {
		window = ui.composer.window
		input = ui.composer.input
		response = ui.composer.response
	}
	var stderrTotal uint64
	if ui.provider != nil {
		stderrTotal = ui.provider.stderrTotal()
	}
	var runInvalid any
	if ui.m.hasRunInvalid {
		runInvalid = ui.m.runInvalid
	}
	var providerError any
	if ui.m.hasProviderErr {
		providerError = ui.m.providerError
	}
	return map[string]any{
		"pid":                    os.Getpid(),
		"mascot_hwnd":            ptrStr(ui.mascotPtr),
		"composer_hwnd":          ptrStr(window),
		"input_hwnd":             ptrStr(input),
		"response_hwnd":          ptrStr(response),
		"composer_visible":       visible,
		"composing":              ui.composing,
		"provider_pid":           ui.m.providerPID,
		"provider_state":         ui.m.providerState,
		"request_id":             ui.m.requestID,
		"request_count":          ui.m.requestCount,
		"last_seq":               ui.m.lastSeq,
		"response_utf8_bytes":    len(ui.m.response),
		"mascot_presents":        ui.presents,
		"composer_paints":        ui.paints,
		"queued_provider_frames": ui.uiEvents.len(),
		"queue_capacity_frames":  ui.uiEvents.cap(),
		"cache_counts": map[string]any{
			"stderr_tail_bytes":            min64(stderrTotal, 4096),
			"stderr_total_bytes":           stderrTotal,
			"retained_response_utf8_bytes": len(ui.m.response),
			"dib_buffers":                  map[string]any{"value": nil, "reason": "no DIB section cache; NSBitmapImageRep owned by NSImage"},
			"richedit_layout_cache_bytes":  map[string]any{"value": nil, "reason": "opaque NSTextView layout cache"},
			"caret_draws":                  map[string]any{"value": nil, "reason": "native caret drawing does not surface through drawRect"},
			"run_invalid":                  runInvalid,
			"provider_error":               providerError,
		},
	}
}

func (ui *UI) textSnapshotJSON() (map[string]any, error) {
	if ui.composer == nil {
		return nil, errors.New("composer not created")
	}
	inputText := mascotTextGet(ui.composer.input)
	full := mascotTextGet(ui.composer.response)
	responseText := full
	if strings.HasPrefix(full, ui.cfg.historyPrefix) {
		responseText = full[len(ui.cfg.historyPrefix):]
	}
	loc, length := mascotSelectionGet(ui.composer.input)
	return map[string]any{
		"input":           inputText,
		"response":        responseText,
		"selection_start": int32(loc),
		"selection_end":   int32(loc + length),
		"composing":       ui.composing,
	}, nil
}

func (ui *UI) setInputText(value string) error {
	if strings.IndexByte(value, 0) >= 0 {
		return errors.New("text contains NUL")
	}
	if utf16Units(value) > int(ui.cfg.manifest.UI.InputLimitUTF16Units) {
		return errors.New("text exceeds input limit")
	}
	if ui.composer == nil {
		return errors.New("composer not created")
	}
	mascotTextSet(ui.composer.input, value)
	loc, length := mascotSelectionGet(ui.composer.input)
	ui.snapshot = &textSnapshot{
		text:     mascotTextGet(ui.composer.input),
		selStart: int32(loc),
		selEnd:   int32(loc + length),
	}
	return nil
}

func (ui *UI) dispatchControl() {
	for i := 0; i < 16; i++ {
		cmd, ok := ui.commands.pop()
		if !ok {
			break
		}
		ui.handleControl(cmd)
	}
}

func rawString(m map[string]json.RawMessage, key string) string {
	raw, ok := m[key]
	if !ok {
		return ""
	}
	var s string
	if err := json.Unmarshal(raw, &s); err != nil {
		return ""
	}
	return s
}

func (ui *UI) reply(token json.RawMessage, withState bool, extra map[string]any) {
	value := map[string]any{"token": tokenOrNull(token), "ok": true}
	if withState {
		value["state"] = ui.stateJSON()
	}
	if extra != nil {
		value["text"] = extra
	}
	ui.record(value)
}

func (ui *UI) replyError(token json.RawMessage, err string) {
	ui.record(map[string]any{"token": tokenOrNull(token), "ok": false, "error": err})
}

func tokenOrNull(token json.RawMessage) json.RawMessage {
	if len(token) == 0 {
		return json.RawMessage("null")
	}
	return token
}

func (ui *UI) handleControl(command map[string]json.RawMessage) {
	token := command["token"]
	name := rawString(command, "command")
	switch name {
	case "state":
		ui.reply(token, true, nil)
	case "text":
		snapshot, err := ui.textSnapshotJSON()
		if err != nil {
			ui.replyError(token, err.Error())
		} else {
			ui.reply(token, true, snapshot)
		}
	case "set_text":
		value := rawString(command, "text")
		if err := ui.setInputText(value); err != nil {
			ui.replyError(token, err.Error())
		} else {
			ui.reply(token, true, nil)
		}
	case "scenario":
		scenario := rawString(command, "name")
		if ui.m.providerState == "streaming" {
			ui.replyError(token, "request active")
		} else if _, ok := ui.cfg.manifest.Scenarios[scenario]; ok {
			ui.m.scenario = scenario
			ui.reply(token, true, nil)
		} else {
			ui.replyError(token, fmt.Sprintf("unknown scenario '%s'", scenario))
		}
	case "show":
		if err := ui.showComposer(); err != nil {
			ui.replyError(token, err.Error())
		} else {
			ui.reply(token, true, nil)
		}
	case "hide":
		ui.hideComposer()
		ui.reply(token, true, nil)
	case "ime_select":
		id := rawString(command, "source")
		status := mascotIMESelect(id)
		ui.reply(token, true, map[string]any{
			"detail": fmt.Sprintf("tis=%d", status),
		})
	case "ime_mark":
		mascotIMEMark(rawString(command, "text"))
		ui.reply(token, true, nil)
	case "ime_insert":
		mascotIMEInsert(rawString(command, "text"))
		ui.reply(token, true, nil)
	case "ime_discard":
		mascotIMEDiscard()
		ui.reply(token, true, nil)
	case "focus":
		bits := mascotFocusInfo()
		ui.reply(token, true, map[string]any{
			"key":      bits&1 != 0,
			"main":     bits&2 != 0,
			"is_input": bits&4 != 0,
			"class":    "",
		})
	case "submit":
		if err := ui.submit(); err != nil {
			ui.replyError(token, err.Error())
		} else {
			ui.reply(token, true, nil)
		}
	case "cancel":
		if err := ui.cancelRequest(); err != nil {
			ui.replyError(token, err.Error())
		} else {
			ui.reply(token, true, nil)
		}
	case "shutdown":
		ui.requestShutdown(&token)
	default:
		ui.replyError(token, fmt.Sprintf("unknown command '%s'", name))
	}
}

func (ui *UI) requestShutdown(token *json.RawMessage) {
	if token != nil {
		ui.pendingShutdownToken = *token
		ui.hasPendingShutdownToken = true
	}
	if ui.shutdownStarted {
		return
	}
	ui.shutdownStarted = true
	ui.hideComposer()
	if ui.provider != nil {
		if err := ui.provider.sendShutdown(); err != nil {
			fmt.Fprintln(os.Stderr, "provider shutdown request failed:", err)
			mascotTerminate()
		}
	} else {
		mascotTerminate()
	}
}

func (ui *UI) teardown() {
	if p := ui.provider; p != nil {
		ui.provider = nil
		if !p.join(3 * time.Second) {
			fmt.Fprintln(os.Stderr, "provider coordinator did not stop within teardown bound")
		}
	}
	ui.commands.close()
	if ui.records != nil {
		ui.records.close()
	}
	if c := ui.composer; c != nil {
		ui.composer = nil
		mascotTeardownComposer(c)
	}
	if ui.mascotPtr != nil {
		mascotTeardownPanel(ui.mascotPtr)
		ui.mascotPtr = nil
	}
}

// ------------------------------------------------------------------ IME glue

// onMarkedChanged mirrors the Windows on_ime_start/end pair: the first marked
// transition snapshots text+selection so hide-during-composition can restore.
func (ui *UI) onMarkedChanged(hasMarked bool) {
	if hasMarked && !ui.composing && ui.composer != nil {
		loc, length := mascotSelectionGet(ui.composer.input)
		ui.snapshot = &textSnapshot{
			text:     mascotTextGet(ui.composer.input),
			selStart: int32(loc),
			selEnd:   int32(loc + length),
		}
	}
	ui.composing = hasMarked
}

func (ui *UI) onUnmark() {
	ui.composing = false
}

// shouldInsert enforces the UTF-16 input limit for delegate-driven changes.
func (ui *UI) shouldInsert(affectedLoc, affectedLen int64, replacement string) bool {
	if ui.composer == nil {
		return true
	}
	current := mascotTextGet(ui.composer.input)
	units := utf16.Encode([]rune(current))
	start := int(affectedLoc)
	if start > len(units) {
		start = len(units)
	}
	l := int(affectedLen)
	if l > len(units)-start {
		l = len(units) - start
	}
	repl := utf16.Encode([]rune(replacement))
	total := len(units) - l + len(repl)
	return total <= int(ui.cfg.manifest.UI.InputLimitUTF16Units)
}

func min64(a, b uint64) uint64 {
	if a < b {
		return a
	}
	return b
}
