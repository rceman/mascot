//go:build darwin

package main

// The macOS fixture is a separate frozen version because the provider binary,
// launch paths and sanitized environment are platform-specific while the
// protocol, payloads, fragmentation plan, cancellation barrier, frame limits
// and non-inheritance contract are identical to windows-v1.0.2.
const fixtureVersion = "macos-v1.0.0"
const manifestFile = "fixture.darwin-arm64.json"
const resultsDir = "results/macos/raw"
