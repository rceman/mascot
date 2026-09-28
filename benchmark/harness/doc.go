//go:build !windows

// Package harness contains the shared Windows measurement foundation. The
// macOS Stage B slice does not use this package; this stub keeps the package
// present for cross-platform `go test ./...` invocations.
package harness
