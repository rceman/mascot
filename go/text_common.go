package main

// Shared text helpers used by both platform UI layers.

import (
	"strings"
	"unicode/utf16"
)

func normalizeText(text string) string {
	text = strings.ReplaceAll(text, "\r\n", "\n")
	return strings.ReplaceAll(text, "\r", "\n")
}

type textSnapshot struct {
	text     string
	selStart int32
	selEnd   int32
}

func utf16Units(text string) int {
	return len(utf16.Encode([]rune(text)))
}

type staticError string

func (e staticError) Error() string { return string(e) }

func errString(s string) error { return staticError(s) }
