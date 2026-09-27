package main

// Entry point: `mascot.exe --fixture MANIFEST [--control]` runs the app on a
// locked UI goroutine; `mascot.exe --decode-vectors VECTORS.json` drives the
// shared framer directly without creating windows or a provider.

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"runtime"
	"time"
	"unsafe"
)

// Kept alive for the whole process so Win32 callback trampolines can resolve
// the UI pointer stored in GWLP_USERDATA.
var theUI *UI

type vectorIn struct {
	Name            string   `json:"name"`
	FragmentsBase64 []string `json:"fragments_base64"`
}

type vectorOut struct {
	Name            string   `json:"name"`
	FramesBase64    []string `json:"frames_base64"`
	Rejected        bool     `json:"rejected"`
	PeakBufferBytes int      `json:"peak_buffer_bytes"`
}

func decodeVectors(path string) (int, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return 0, fmt.Errorf("read vectors: %w", err)
	}
	var vectors []vectorIn
	if err := json.Unmarshal(data, &vectors); err != nil {
		return 0, fmt.Errorf("parse vectors: %w", err)
	}
	for _, vector := range vectors {
		decoder := newFrameDecoder()
		var frames []string
		rejected := false
		for _, fragment := range vector.FragmentsBase64 {
			bytes, err := base64.StdEncoding.DecodeString(fragment)
			if err != nil {
				return 0, fmt.Errorf("vector %s fragment base64: %w", vector.Name, err)
			}
			if err := decoder.feed(bytes, qpc, func(frame []byte, _ int64) error {
				frames = append(frames, base64.StdEncoding.EncodeToString(append([]byte(nil), frame...)))
				return nil
			}); err != nil {
				rejected = true
				break
			}
		}
		if !rejected && decoder.finish() != nil {
			rejected = true
		}
		row, err := json.Marshal(vectorOut{
			Name:            vector.Name,
			FramesBase64:    frames,
			Rejected:        rejected,
			PeakBufferBytes: decoder.peakBufferBytes(),
		})
		if err != nil {
			return 0, err
		}
		if _, err := os.Stdout.Write(append(row, '\n')); err != nil {
			return 0, fmt.Errorf("stdout: %w", err)
		}
	}
	return 0, nil
}

func setDPIAwareness() error {
	r, _, _ := procSetProcessDpiAwarenessContext.Call(dpiAwarenessContextPerMonitorAwareV2)
	if r == 0 {
		current, _, _ := procGetThreadDpiAwarenessContext.Call()
		eq, _, _ := procAreDpiAwarenessContextsEqual.Call(current, dpiAwarenessContextPerMonitorAwareV2)
		if eq == 0 {
			return errors.New("per-monitor-v2 awareness not in effect")
		}
	}
	return nil
}

func messageLoop(ui *UI) int {
	var m msg
	for {
		r, _, _ := procGetMessageW.Call(uintptr(unsafe.Pointer(&m)), 0, 0, 0)
		if r == 0 {
			return int(m.WParam)
		}
		if int64(r) < 0 {
			fmt.Fprintln(os.Stderr, "GetMessageW failed; requesting backend stop")
			ui.requestShutdown(nil)
			return 64
		}
		if ui.consumeSubmitKey(&m) {
			continue
		}
		procTranslateMessage.Call(uintptr(unsafe.Pointer(&m)))
		procDispatchMessageW.Call(uintptr(unsafe.Pointer(&m)))
	}
}

func runApp(manifestArg string, controlMode bool) (int, error) {
	// The UI thread owns all windows and the model; it must stay on one OS
	// thread for COM/OLE, subclass references and the message queue.
	runtime.LockOSThread()
	if err := setDPIAwareness(); err != nil {
		return 0, err
	}
	cfg, err := loadConfig(manifestArg)
	if err != nil {
		return 0, err
	}
	mascotSource, srcW, srcH, err := decodePNG(cfg.assetPath)
	if err != nil {
		return 0, err
	}
	if srcW != int(cfg.manifest.Asset.PixelWidth) || srcH != int(cfg.manifest.Asset.PixelHeight) {
		return 0, errors.New("decoded asset dimensions differ from the manifest")
	}
	ui := &UI{
		cfg:           cfg,
		mascotSource:  mascotSource,
		mascotSrcW:    srcW,
		mascotSrcH:    srcH,
		uiEvents:      newBoundedQueue[providerEvent](64),
		commands:      newBoundedQueue[map[string]json.RawMessage](16),
		controlEnabled: controlMode,
	}
	ui.m.providerState = "idle"
	ui.m.lastSeq = -1
	ui.m.scenario = "normal"
	if controlMode {
		ui.records = newBoundedQueue[string](256)
	}
	theUI = ui

	instance := moduleInstance()
	if err := registerClasses(instance); err != nil {
		return 0, err
	}
	mascot, err := createMascot(ui, instance)
	if err != nil {
		return 0, err
	}
	ui.mascotHwnd = mascot
	dpi := getDpiForWindow(mascot)
	if err := ui.presentMascot(dpi); err != nil {
		return 0, err
	}
	procShowWindow.Call(mascot, swSHOWNOACTIVATE)
	if r, _, _ := procRegisterHotKey.Call(mascot, hotkeyToggleID, modNOREPEAT|modCONTROL|modALT, vkSPACE); r == 0 {
		return 0, errors.New("hotkey registration failed")
	}
	if r, _, _ := procRegisterHotKey.Call(mascot, hotkeyCancelID, modNOREPEAT|modCONTROL|modALT, vkESCAPE); r == 0 {
		return 0, errors.New("cancel hotkey registration failed")
	}

	chunksByScenario := make(map[string]uint64)
	for name := range cfg.manifest.Scenarios {
		if chunks, err := scenarioChunks(cfg.manifest, name); err == nil {
			chunksByScenario[name] = chunks
		}
	}
	hwnd := mascot
	wake := func() { postMessage(hwnd, wmAPPPROVIDER, 0, 0) }
	ui.provider = spawnProvider(providerConfig{
		path:            cfg.manifest.Provider.Path,
		arguments:       cfg.manifest.Provider.Arguments,
		cwd:             cfg.manifest.Provider.Cwd,
		environment:     cfg.manifest.Provider.Environment,
		scenarioChunks:  chunksByScenario,
		cancelTimeout:   durationMs(cfg.manifest.Protocol.CancelTimeoutMs),
		shutdownTimeout: durationMs(cfg.manifest.Protocol.ShutdownTimeoutMs),
	}, ui.uiEvents, ui.records, wake)

	ctl := &control{}
	ctl.arm(ui, controlMode)

	exitCode := messageLoop(ui)
	ctl.stopAll()
	ui.teardown()
	procUnregisterHotKey.Call(mascot, hotkeyToggleID)
	procUnregisterHotKey.Call(mascot, hotkeyCancelID)
	return exitCode, nil
}

func durationMs(ms uint64) time.Duration {
	return time.Duration(ms) * time.Millisecond
}

func main() {
	args := os.Args
	var code int
	var err error
	switch {
	case len(args) > 1 && args[1] == "--decode-vectors":
		if len(args) <= 2 {
			err = errors.New("missing vectors path")
		} else {
			code, err = decodeVectors(args[2])
		}
	case len(args) > 1 && args[1] == "--codex-gate":
		if len(args) <= 2 {
			err = errors.New("missing codex executable path")
		} else {
			err = codexGate(args[2])
			if err != nil {
				gateLine("result", "FAIL")
				fmt.Fprintln(os.Stderr, "codex-gate:", err)
				os.Exit(1)
			}
			gateLine("result", "PASS")
		}
	case len(args) > 1 && args[1] == "--fixture":
		controlMode := false
		for _, a := range args[2:] {
			if a == "--control" {
				controlMode = true
			}
		}
		if len(args) <= 2 {
			err = errors.New("missing manifest path")
		} else {
			code, err = runApp(args[2], controlMode)
		}
	default:
		err = errors.New("usage: mascot --fixture MANIFEST [--control] | --decode-vectors VECTORS")
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(64)
	}
	os.Exit(code)
}
