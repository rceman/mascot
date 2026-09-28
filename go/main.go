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
	"time"
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
	case len(args) > 1 && args[1] == "--acp-gate":
		if len(args) <= 2 {
			err = errors.New("missing ACP executable path")
		} else {
			err = acpGate(args[2])
			if err != nil {
				acpLine("result", "FAIL")
				fmt.Fprintln(os.Stderr, "acp-gate:", err)
				os.Exit(1)
			}
			acpLine("result", "PASS")
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
		err = errors.New("usage: mascot --fixture MANIFEST [--control] | --decode-vectors VECTORS | --acp-gate ACPEXE")
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(64)
	}
	os.Exit(code)
}
