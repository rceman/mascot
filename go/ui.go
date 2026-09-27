package main

// UI ownership: the locked UI goroutine owns every HWND, the model, the
// composer and the mascot surface. Provider events arrive on the bounded
// uiEvents queue and are dispatched on the UI thread via WM_APP_PROVIDER wake
// posts; control commands similarly via WM_APP_CONTROL. No other goroutine
// touches HWND-owned state.

import (
	"encoding/json"
	"errors"
	"fmt"
	"image"
	"image/draw"
	"image/png"
	"os"
	"strconv"
	"strings"
	"time"
	"unsafe"
)

const (
	idSend   = 101
	idCancel = 102
	idStatus = 103
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

type surface struct {
	dc       uintptr
	bitmap   uintptr
	previous uintptr
	pixels   int
}

type composer struct {
	hwnd     uintptr
	input    uintptr
	response uintptr
	status   uintptr
	send     uintptr
	cancel   uintptr
	font     uintptr
	module   uintptr
	dpi      uint32
}

type UI struct {
	cfg                     *config
	mascotHwnd              uintptr
	mascotSource            []byte // premultiplied BGRA, 128x128
	surface                 *surface
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

func decodePNG(path string) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open asset: %w", err)
	}
	defer f.Close()
	img, err := png.Decode(f)
	if err != nil {
		return nil, fmt.Errorf("png decode: %w", err)
	}
	if img.Bounds().Dx() != 128 || img.Bounds().Dy() != 128 {
		return nil, errors.New("asset pixel size mismatch")
	}
	nrgba, ok := img.(*image.NRGBA)
	if !ok {
		nrgba = image.NewNRGBA(image.Rect(0, 0, 128, 128))
		draw.Draw(nrgba, nrgba.Bounds(), img, img.Bounds().Min, draw.Src)
	}
	rgba := nrgba.Pix
	premul := make([]byte, len(rgba))
	for i := 0; i < len(rgba); i += 4 {
		r, g, b, a := uint32(rgba[i]), uint32(rgba[i+1]), uint32(rgba[i+2]), uint32(rgba[i+3])
		premul[i] = byte((b*a + 127) / 255)
		premul[i+1] = byte((g*a + 127) / 255)
		premul[i+2] = byte((r*a + 127) / 255)
		premul[i+3] = byte(a)
	}
	return premul, nil
}

func newSurface(source []byte, pixels int) (*surface, error) {
	screen, _, _ := procGetDC.Call(0)
	if screen == 0 {
		return nil, errString("GetDC failed")
	}
	defer procReleaseDC.Call(0, screen)
	memory, _, _ := procCreateCompatibleDC.Call(screen)
	if memory == 0 {
		return nil, errString("CreateCompatibleDC failed")
	}
	info := bitmapinfo{}
	info.BmiHeader.BiSize = uint32(unsafe.Sizeof(info.BmiHeader))
	info.BmiHeader.BiWidth = int32(pixels)
	info.BmiHeader.BiHeight = -int32(pixels) // top-down DIB
	info.BmiHeader.BiPlanes = 1
	info.BmiHeader.BiBitCount = 32
	info.BmiHeader.BiCompression = biRGB
	var bits uintptr
	bitmap, _, _ := procCreateDIBSection.Call(
		screen,
		uintptr(unsafe.Pointer(&info)),
		dibRGBCOLORS,
		uintptr(unsafe.Pointer(&bits)),
		0,
		0,
	)
	if bitmap == 0 || bits == 0 {
		if bitmap != 0 {
			procDeleteObject.Call(bitmap)
		}
		procDeleteDC.Call(memory)
		return nil, errString("CreateDIBSection failed")
	}
	target := unsafe.Slice((*byte)(unsafe.Pointer(bits)), pixels*pixels*4)
	for y := 0; y < pixels; y++ {
		for x := 0; x < pixels; x++ {
			sx := x * 128 / pixels
			sy := y * 128 / pixels
			src := (sy*128 + sx) * 4
			dst := (y*pixels + x) * 4
			copy(target[dst:dst+4], source[src:src+4])
		}
	}
	previous, _, _ := procSelectObject.Call(memory, bitmap)
	if previous == 0 {
		procDeleteObject.Call(bitmap)
		procDeleteDC.Call(memory)
		return nil, errString("SelectObject failed")
	}
	return &surface{dc: memory, bitmap: bitmap, previous: previous, pixels: pixels}, nil
}

func (s *surface) destroy() {
	procSelectObject.Call(s.dc, s.previous)
	procDeleteObject.Call(s.bitmap)
	procDeleteDC.Call(s.dc)
}

func (ui *UI) presentMascot(dpi uint32) error {
	pixels := dip(int32(ui.cfg.manifest.Asset.LogicalWidthDip), dpi)
	surf, err := newSurface(ui.mascotSource, int(pixels))
	if err != nil {
		return err
	}
	if ui.surface != nil {
		ui.surface.destroy()
	}
	ui.surface = surf
	var bounds rect
	if r, _, _ := procGetWindowRect.Call(ui.mascotHwnd, uintptr(unsafe.Pointer(&bounds))); r == 0 {
		return errString("GetWindowRect failed")
	}
	topLeft := point{X: bounds.Left, Y: bounds.Top}
	sz := size{CX: int32(surf.pixels), CY: int32(surf.pixels)}
	zero := point{}
	blend := blendfunction{BlendOp: acSRCOVER, SourceConstantAlpha: 255, AlphaFormat: acSRCALPHA}
	screen, _, _ := procGetDC.Call(0)
	if screen == 0 {
		return errString("GetDC failed")
	}
	presented, _, _ := procUpdateLayeredWindow.Call(
		ui.mascotHwnd,
		screen,
		uintptr(unsafe.Pointer(&topLeft)),
		uintptr(unsafe.Pointer(&sz)),
		surf.dc,
		uintptr(unsafe.Pointer(&zero)),
		0,
		uintptr(unsafe.Pointer(&blend)),
		ulwALPHA,
	)
	procReleaseDC.Call(0, screen)
	if presented == 0 {
		return errString("UpdateLayeredWindow failed")
	}
	ui.presents++
	return nil
}

func (ui *UI) composerHandles() (hwnd, input, response, status, send, cancel uintptr, ok bool) {
	c := ui.composer
	if c == nil {
		return 0, 0, 0, 0, 0, 0, false
	}
	return c.hwnd, c.input, c.response, c.status, c.send, c.cancel, true
}

func getDpiForWindow(hwnd uintptr) uint32 {
	r, _, _ := procGetDpiForWindow.Call(hwnd)
	return uint32(r)
}

func max32(a, b int32) int32 {
	if a > b {
		return a
	}
	return b
}

func clamp(v, lo, hi int32) int32 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func (ui *UI) ensureComposer() (uintptr, error) {
	if hwnd, _, _, _, _, _, ok := ui.composerHandles(); ok {
		return hwnd, nil
	}
	instance := moduleInstance()
	mascot := ui.mascotHwnd
	dpi := getDpiForWindow(mascot)
	r := rect{
		Right:  dip(int32(ui.cfg.manifest.UI.ComposerClientWidthDip), dpi),
		Bottom: dip(int32(ui.cfg.manifest.UI.ComposerClientHeightDip), dpi),
	}
	style := uint32(wsCAPTION | wsSYSMENU | wsMINIMIZEBOX | wsCLIPCHILDREN)
	procAdjustWindowRectExForDpi.Call(
		uintptr(unsafe.Pointer(&r)), uintptr(style), 0, 0, uintptr(dpi))
	var mascotRect rect
	procGetWindowRect.Call(mascot, uintptr(unsafe.Pointer(&mascotRect)))
	place := point{X: mascotRect.Right + 8, Y: mascotRect.Top}
	monitor, _, _ := procMonitorFromWindow.Call(mascot, monitorDEFAULTTONEAREST)
	mi := monitorinfo{CbSize: uint32(unsafe.Sizeof(monitorinfo{}))}
	procGetMonitorInfoW.Call(monitor, uintptr(unsafe.Pointer(&mi)))
	width := r.Right - r.Left
	height := r.Bottom - r.Top
	place.X = clamp(place.X, mi.RcWork.Left, max32(mi.RcWork.Right-width, mi.RcWork.Left))
	place.Y = clamp(place.Y, mi.RcWork.Top, max32(mi.RcWork.Bottom-height, mi.RcWork.Top))
	hwnd, _, _ := procCreateWindowExW.Call(
		0,
		uintptr(unsafe.Pointer(widep("MascotGoComposer"))),
		uintptr(unsafe.Pointer(widep("mascot"))),
		uintptr(style),
		uintptr(place.X), uintptr(place.Y), uintptr(width), uintptr(height),
		0, 0, instance,
		uintptr(unsafe.Pointer(ui)),
	)
	if hwnd == 0 {
		return 0, errors.New("composer creation failed")
	}
	ui.enforceComposerClient(hwnd)
	return hwnd, nil
}

func (ui *UI) enforceComposerClient(hwnd uintptr) {
	dpi := getDpiForWindow(hwnd)
	var r rect
	procGetClientRect.Call(hwnd, uintptr(unsafe.Pointer(&r)))
	wantW := dip(int32(ui.cfg.manifest.UI.ComposerClientWidthDip), dpi)
	wantH := dip(int32(ui.cfg.manifest.UI.ComposerClientHeightDip), dpi)
	if r.Right-r.Left == wantW && r.Bottom-r.Top == wantH {
		return
	}
	style := uint32(wsCAPTION | wsSYSMENU | wsMINIMIZEBOX | wsCLIPCHILDREN)
	frame := rect{Right: wantW, Bottom: wantH}
	procAdjustWindowRectExForDpi.Call(
		uintptr(unsafe.Pointer(&frame)), uintptr(style), 0, 0, uintptr(dpi))
	var origin rect
	procGetWindowRect.Call(hwnd, uintptr(unsafe.Pointer(&origin)))
	procSetWindowPos.Call(
		hwnd, 0,
		uintptr(origin.Left), uintptr(origin.Top),
		uintptr(frame.Right-frame.Left), uintptr(frame.Bottom-frame.Top),
		swpNOZORDER|swpNOACTIVATE,
	)
}

func (ui *UI) showComposer() error {
	hwnd, err := ui.ensureComposer()
	if err != nil {
		return err
	}
	_, input, _, _, _, _, _ := ui.composerHandles()
	procShowWindow.Call(hwnd, swSHOWNORMAL)
	procSetForegroundWindow.Call(hwnd)
	if input != 0 {
		procSetFocus.Call(input)
	}
	return nil
}

func (ui *UI) hideComposer() {
	hwnd, input, _, _, _, _, ok := ui.composerHandles()
	if !ok {
		return
	}
	if ui.composing {
		cancelComposition(input)
		if snap := ui.snapshot; snap != nil {
			setText(input, snap.text)
			setSelection(input, snap.selStart, snap.selEnd)
		}
		ui.composing = false
	}
	procShowWindow.Call(hwnd, swHIDE)
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
	prompt := getText(ui.composer.input)
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
	if _, _, response, _, _, _, ok := ui.composerHandles(); ok {
		setText(response, ui.cfg.historyPrefix)
	}
}

func (ui *UI) appendResponseView(text string) {
	if _, _, response, _, _, _, ok := ui.composerHandles(); ok {
		appendText(response, text)
	}
}

func (ui *UI) refreshStatus() {
	if _, _, _, status, _, _, ok := ui.composerHandles(); ok {
		procSetWindowTextW.Call(status, uintptr(unsafe.Pointer(widep(ui.m.providerState))))
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
			code := uintptr(0)
			if ev.hasErr {
				code = 1
			}
			procPostQuitMessage.Call(code)
		}
	}
}

func (ui *UI) stateJSON() map[string]any {
	hwnd, input, response, _, _, _, ok := ui.composerHandles()
	visible := false
	if ok {
		r, _, _ := procIsWindowVisible.Call(hwnd)
		visible = r != 0
	}
	hwndStr := func(h uintptr) string {
		if !ok {
			return "0"
		}
		return strconv.FormatUint(uint64(h), 10)
	}
	var stderrTotal uint64
	if ui.provider != nil {
		stderrTotal = ui.provider.stderrTotal()
	}
	dib := uint64(0)
	if ui.surface != nil {
		dib = 1
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
		"mascot_hwnd":            strconv.FormatUint(uint64(ui.mascotHwnd), 10),
		"composer_hwnd":          hwndStr(hwnd),
		"input_hwnd":             hwndStr(input),
		"response_hwnd":          hwndStr(response),
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
			"stderr_tail_bytes":              min64(stderrTotal, 4096),
			"stderr_total_bytes":             stderrTotal,
			"retained_response_utf8_bytes":   len(ui.m.response),
			"dib_buffers":                    dib,
			"richedit_layout_cache_bytes":    map[string]any{"value": nil, "reason": "opaque system RichEdit layout cache"},
			"caret_draws":                    map[string]any{"value": nil, "reason": "native caret drawing does not surface through WM_PAINT"},
			"run_invalid":                    runInvalid,
			"provider_error":                 providerError,
		},
	}
}

func (ui *UI) textSnapshotJSON() (map[string]any, error) {
	_, input, response, _, _, _, ok := ui.composerHandles()
	if !ok {
		return nil, errors.New("composer not created")
	}
	inputText := getText(input)
	full := getText(response)
	responseText := full
	if strings.HasPrefix(full, ui.cfg.historyPrefix) {
		responseText = full[len(ui.cfg.historyPrefix):]
	}
	rng := getSelection(input)
	return map[string]any{
		"input":           inputText,
		"response":        responseText,
		"selection_start": rng.Min,
		"selection_end":   rng.Max,
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
	_, input, _, _, _, _, ok := ui.composerHandles()
	if !ok {
		return errors.New("composer not created")
	}
	setText(input, value)
	rng := getSelection(input)
	ui.snapshot = &textSnapshot{text: getText(input), selStart: rng.Min, selEnd: rng.Max}
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
			procPostQuitMessage.Call(64)
		}
	} else {
		procPostQuitMessage.Call(0)
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
		procDestroyWindow.Call(c.input)
		procDestroyWindow.Call(c.response)
		procDestroyWindow.Call(c.status)
		procDestroyWindow.Call(c.send)
		procDestroyWindow.Call(c.cancel)
		procDestroyWindow.Call(c.hwnd)
		procDeleteObject.Call(c.font)
		procFreeLibrary.Call(c.module)
		procOleUninitialize.Call()
	}
	if s := ui.surface; s != nil {
		ui.surface = nil
		s.destroy()
	}
	if ui.mascotHwnd != 0 {
		procDestroyWindow.Call(ui.mascotHwnd)
		ui.mascotHwnd = 0
	}
}

// consumeSubmitKey intercepts Ctrl+Enter targeted at the input control before
// Translate/Dispatch so it submits exactly once when not composing.
func (ui *UI) consumeSubmitKey(m *msg) bool {
	if m.Message != wmKEYDOWN || uint16(m.WParam) != vkRETURN {
		return false
	}
	var input uintptr
	if ui.composer != nil {
		input = ui.composer.input
	}
	if input == 0 || m.Hwnd != input {
		return false
	}
	r, _, _ := procGetKeyState.Call(vkCONTROL)
	if int16(r) >= 0 {
		return false
	}
	if ui.composing {
		return false
	}
	ui.postSubmit()
	return true
}

func (ui *UI) postSubmit() {
	if ui.mascotHwnd != 0 {
		postMessage(ui.mascotHwnd, wmAPPSUBMIT, 0, 0)
	}
}

func (ui *UI) onIMEStart(input uintptr) {
	ui.composing = true
	rng := getSelection(input)
	ui.snapshot = &textSnapshot{text: getText(input), selStart: rng.Min, selEnd: rng.Max}
}

func (ui *UI) onIMEEnd(input uintptr) {
	ui.composing = false
}

func (ui *UI) onIMEUpdate() {
	ui.composing = true
}
