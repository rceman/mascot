package main

// macOS application entry: locked main thread owns the NSApplication run
// loop, the AppKit windows and the UI model. Mirrors runapp_windows.go.

import (
	"encoding/json"
	"errors"
	"runtime"
)

func runApp(manifestArg string, controlMode bool) (int, error) {
	runtime.LockOSThread()
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
		cfg:            cfg,
		mascotSource:   mascotSource,
		mascotSrcW:     srcW,
		mascotSrcH:     srcH,
		uiEvents:       newBoundedQueue[providerEvent](64),
		commands:       newBoundedQueue[map[string]json.RawMessage](16),
		controlEnabled: controlMode,
	}
	ui.m.providerState = "idle"
	ui.m.lastSeq = -1
	ui.m.scenario = "normal"
	if controlMode {
		ui.records = newBoundedQueue[string](256)
	}
	theUI = ui

	if !mascotAppInit() {
		return 0, errors.New("NSApplication initialization failed")
	}
	panel := mascotCreateMascot(mascotSource, srcW, srcH,
		float64(cfg.manifest.Asset.LogicalWidthDip))
	if panel == nil {
		return 0, errors.New("mascot window creation failed")
	}
	ui.mascotPtr = panel
	mascotOrderFront(panel)
	if !mascotRegisterHotkeys() {
		return 0, errors.New("hotkey registration failed")
	}

	chunksByScenario := make(map[string]uint64)
	for name := range cfg.manifest.Scenarios {
		if chunks, serr := scenarioChunks(cfg.manifest, name); serr == nil {
			chunksByScenario[name] = chunks
		}
	}
	ui.provider = spawnProvider(providerConfig{
		path:            cfg.manifest.Provider.Path,
		arguments:       cfg.manifest.Provider.Arguments,
		cwd:             cfg.manifest.Provider.Cwd,
		environment:     cfg.manifest.Provider.Environment,
		scenarioChunks:  chunksByScenario,
		cancelTimeout:   durationMs(cfg.manifest.Protocol.CancelTimeoutMs),
		shutdownTimeout: durationMs(cfg.manifest.Protocol.ShutdownTimeoutMs),
	}, ui.uiEvents, ui.records, ui.postProviderWake)

	ctl := &control{}
	ctl.arm(ui, controlMode)

	mascotRun()
	ctl.stopAll()
	ui.teardown()
	mascotUnregisterHotkeys()
	return 0, nil
}
