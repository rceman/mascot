package main

import "os/exec"

// No console-window suppression needed on macOS.
func applyProviderSysProcAttr(cmd *exec.Cmd) {}
