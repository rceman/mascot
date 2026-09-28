package main

import (
	"errors"
	"fmt"
	"strings"
)

func gateLine(step, detail string) {
	clean := strings.Map(func(r rune) rune {
		if r == '"' || r == '\\' || r < ' ' {
			return ' '
		}
		return r
	}, detail)
	fmt.Printf("{\"event\":\"codex_gate\",\"step\":\"%s\",\"detail\":\"%s\"}\n", step, clean)
}

// The Codex app-server gate is Windows-scoped. macOS Stage B uses the Devin
// ACP gate (--acp-gate) instead.
func codexGate(codexExe string) error {
	return errors.New("codex gate unsupported on macOS; use --acp-gate")
}
