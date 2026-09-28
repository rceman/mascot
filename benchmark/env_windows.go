//go:build windows

package main

func providerEnvironment() map[string]string {
	return map[string]string{
		"SystemRoot": `C:\Windows`, "WINDIR": `C:\Windows`, "PATH": `C:\Windows\System32`,
		"GOMAXPROCS": "1", "GOGC": "100", "GOMEMLIMIT": "off", "GODEBUG": "", "GOTRACEBACK": "none",
	}
}

func environmentList() []string {
	values := providerEnvironment()
	keys := []string{"SystemRoot", "WINDIR", "PATH", "GOMAXPROCS", "GOGC", "GOMEMLIMIT", "GODEBUG", "GOTRACEBACK"}
	result := make([]string, 0, len(keys))
	for _, key := range keys {
		result = append(result, key+"="+values[key])
	}
	return result
}
