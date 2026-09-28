package main

// Windows application entry: DPI awareness, class registration, mascot
// creation/presentation, hotkeys, provider spawn, control arm and the modal
// message loop. Mirrors the platform boundary used by the darwin port.

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

