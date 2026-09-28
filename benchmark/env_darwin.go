//go:build darwin

package main

import "os"

// The sanitized darwin provider environment keeps the same contract as the
// frozen Windows map: an explicit eight-key environment with no inheritance,
// including the identical Go runtime controls. PATH, HOME and LANG replace the
// Windows-only SystemRoot/WINDIR keys; values are recorded verbatim in the
// macOS manifest.
func providerEnvironment() map[string]string {
	home, err := os.UserHomeDir()
	if err != nil || home == "" {
		home = "/var/empty"
	}
	return map[string]string{
		"PATH": "/usr/bin:/bin", "HOME": home, "LANG": "en_US.UTF-8",
		"GOMAXPROCS": "1", "GOGC": "100", "GOMEMLIMIT": "off", "GODEBUG": "", "GOTRACEBACK": "none",
	}
}

func environmentList() []string {
	values := providerEnvironment()
	keys := []string{"PATH", "HOME", "LANG", "GOMAXPROCS", "GOGC", "GOMEMLIMIT", "GODEBUG", "GOTRACEBACK"}
	result := make([]string, 0, len(keys))
	for _, key := range keys {
		result = append(result, key+"="+values[key])
	}
	return result
}
