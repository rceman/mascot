package main

// Fixture manifest loading and validation, mirroring rust/src/config.rs.
// Validation fails closed on any frozen-field drift.

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type manifestAsset struct {
	Path             string `json:"path"`
	PixelWidth       uint32 `json:"pixel_width"`
	PixelHeight      uint32 `json:"pixel_height"`
	LogicalWidthDip  uint32 `json:"logical_width_dip"`
	LogicalHeightDip uint32 `json:"logical_height_dip"`
	SHA256           string `json:"sha256"`
}

type manifestProvider struct {
	Path               string            `json:"path"`
	Arguments          []string          `json:"arguments"`
	Cwd                string            `json:"cwd"`
	Environment        map[string]string `json:"environment"`
	InheritEnvironment bool              `json:"inherit_environment"`
}

type manifestProtocol struct {
	MaxStdoutFrameBytesIncludingLF uint64 `json:"max_stdout_frame_bytes_including_lf"`
	MaxStdinFrameBytesIncludingLF  uint64 `json:"max_stdin_frame_bytes_including_lf"`
	CancelTimeoutMs                uint64 `json:"cancel_timeout_ms"`
	ShutdownTimeoutMs              uint64 `json:"shutdown_timeout_ms"`
	NormalChunks                   uint64 `json:"normal_chunks"`
}

type manifestUI struct {
	ComposerClientWidthDip  uint32 `json:"composer_client_width_dip"`
	ComposerClientHeightDip uint32 `json:"composer_client_height_dip"`
	InputHeightDip          uint32 `json:"input_height_dip"`
	ResponseHeightDip       uint32 `json:"response_height_dip"`
	MarginDip               uint32 `json:"margin_dip"`
	Hotkey                  string `json:"hotkey"`
	Submit                  string `json:"submit"`
	Cancel                  string `json:"cancel"`
	InputLimitUTF16Units    uint32 `json:"input_limit_utf16_units"`
	ResponseLimitUTF8Bytes  uint32 `json:"response_limit_utf8_bytes"`
	HistoryMessages         uint32 `json:"history_messages"`
	HideCancelsRequest      bool   `json:"hide_cancels_request"`
}

type manifest struct {
	Version   string                     `json:"version"`
	Asset     manifestAsset              `json:"asset"`
	Provider  manifestProvider           `json:"provider"`
	Protocol  manifestProtocol           `json:"protocol"`
	Scenarios map[string]json.RawMessage `json:"scenarios"`
	UI        manifestUI                 `json:"ui"`
}

type config struct {
	manifest      *manifest
	assetPath     string
	historyPrefix string
}

func sha256File(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:]), nil
}

func expectedChunks(scenario string, m *manifest) uint64 {
	switch scenario {
	case "backpressure":
		return 256
	case "maximum", "oversized":
		return 1
	default:
		return m.Protocol.NormalChunks
	}
}

func scenarioChunks(m *manifest, scenario string) (uint64, error) {
	if _, ok := m.Scenarios[scenario]; !ok {
		return 0, fmt.Errorf("unknown scenario '%s'", scenario)
	}
	return expectedChunks(scenario, m), nil
}

func loadConfig(manifestArg string) (*config, error) {
	manifestPath, err := filepath.Abs(manifestArg)
	if err != nil {
		return nil, fmt.Errorf("manifest path: %w", err)
	}
	if resolved, rerr := filepath.EvalSymlinks(manifestPath); rerr == nil {
		manifestPath = resolved
	}
	data, err := os.ReadFile(manifestPath)
	if err != nil {
		return nil, fmt.Errorf("read manifest: %w", err)
	}
	var m manifest
	if err := json.Unmarshal(data, &m); err != nil {
		return nil, fmt.Errorf("parse manifest: %w", err)
	}
	root := filepath.Dir(filepath.Dir(manifestPath))
	if root == "" {
		return nil, errors.New("manifest has no benchmark root")
	}

	if m.Asset.PixelWidth == 0 || m.Asset.PixelHeight == 0 {
		return nil, errors.New("asset pixel size missing")
	}
	if m.Asset.LogicalWidthDip != 64 || m.Asset.LogicalHeightDip != 64 {
		return nil, errors.New("unsupported asset logical size")
	}
	if m.Protocol.MaxStdoutFrameBytesIncludingLF != 65536 ||
		m.Protocol.MaxStdinFrameBytesIncludingLF != 65536 {
		return nil, errors.New("unsupported frame limits")
	}
	if m.Provider.InheritEnvironment {
		return nil, errors.New("provider must not inherit environment")
	}
	if !strings.HasPrefix(m.Version, "windows-v") {
		return nil, fmt.Errorf("unsupported fixture version %s", m.Version)
	}
	ui := &m.UI
	if ui.ComposerClientWidthDip != 640 || ui.ComposerClientHeightDip != 480 ||
		ui.InputHeightDip != 128 || ui.ResponseHeightDip != 272 || ui.MarginDip != 12 {
		return nil, errors.New("unsupported composer geometry")
	}
	if ui.Hotkey != "CTRL+ALT+SPACE" || ui.Submit != "CTRL+ENTER" || ui.Cancel != "CTRL+ALT+ESCAPE" {
		return nil, errors.New("unsupported hotkey mapping")
	}
	if ui.HideCancelsRequest {
		return nil, errors.New("hide must not cancel the provider request")
	}
	for _, required := range []string{
		"normal", "cancel", "client_request", "backpressure", "maximum",
		"oversized", "unexpected_exit", "stderr", "fragmented",
	} {
		if _, ok := m.Scenarios[required]; !ok {
			return nil, fmt.Errorf("manifest missing scenario '%s'", required)
		}
	}

	assetPath := filepath.Join(root, filepath.FromSlash(m.Asset.Path))
	actual, err := sha256File(assetPath)
	if err != nil {
		return nil, err
	}
	if !strings.EqualFold(actual, m.Asset.SHA256) {
		return nil, fmt.Errorf("asset sha256 mismatch: %s", actual)
	}

	historyPath := filepath.Join(root, "fixtures", "history.json")
	historyData, err := os.ReadFile(historyPath)
	if err != nil {
		return nil, fmt.Errorf("read history: %w", err)
	}
	var history []string
	if err := json.Unmarshal(historyData, &history); err != nil {
		return nil, fmt.Errorf("parse history: %w", err)
	}
	if uint32(len(history)) != m.UI.HistoryMessages {
		return nil, errors.New("history length does not match manifest")
	}
	historyPrefix := strings.Join(history, "\n") + "\n\n"

	return &config{manifest: &m, assetPath: assetPath, historyPrefix: historyPrefix}, nil
}
