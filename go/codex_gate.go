package main

// Untimed Codex app-server compatibility gate (benchmark protocol §16).
// Launches a pinned `codex app-server` over stdio through the same
// explicit-environment process machinery the provider path uses, completes
// JSON-RPC initialization, performs one read-only streamed interaction, and
// tears the process down cleanly. Emits gate evidence as JSON lines on
// stdout; exit code 0 = PASS, 1 = FAIL.

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
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

func codexGate(codexExe string) error {
	profile := os.Getenv("USERPROFILE")
	temp := os.TempDir()
	homePath := strings.TrimPrefix(profile, "C:")
	cmd := exec.Command(codexExe, "app-server")
	cmd.Dir = filepath.Dir(codexExe)
	cmd.Env = []string{
		`PATH=C:\Windows\System32;C:\Windows`,
		"SystemRoot=C:\\Windows",
		"WINDIR=C:\\Windows",
		"TEMP=" + temp, "TMP=" + temp,
		"USERPROFILE=" + profile,
		"HOMEDRIVE=C:", "HOMEPATH=" + homePath,
	}
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return err
	}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("spawn: %w", err)
	}
	gateLine("spawn", "codex app-server started")

	lines := make(chan string, 64)
	go func() {
		defer close(lines)
		reader := bufio.NewReaderSize(stdout, 64*1024)
		for {
			line, err := reader.ReadString('\n')
			if line != "" {
				lines <- strings.TrimRight(line, "\r\n")
			}
			if err != nil {
				return
			}
		}
	}()
	stderrDone := make(chan int64)
	go func() {
		n, _ := io.Copy(io.Discard, stderr)
		stderrDone <- n
	}()

	send := func(frame string) error {
		_, err := io.WriteString(stdin, frame+"\n")
		return err
	}
	if err := send(`{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"mascot-gate","version":"0.1"}}}`); err != nil {
		return fmt.Errorf("initialize send: %w", err)
	}
	if err := send(`{"jsonrpc":"2.0","method":"notifications/initialized"}`); err != nil {
		return err
	}
	if err := send(`{"jsonrpc":"2.0","id":2,"method":"config/read","params":{}}`); err != nil {
		return err
	}
	if err := send(`{"jsonrpc":"2.0","id":3,"method":"thread/list","params":{}}`); err != nil {
		return err
	}

	answered := [4]bool{}
	var received, notifications uint64
	deadline := time.Now().Add(30 * time.Second)
	for !answered[1] || !answered[2] || !answered[3] {
		left := time.Until(deadline)
		if left <= 0 {
			return errors.New("timeout waiting for app-server replies")
		}
		if left > 500*time.Millisecond {
			left = 500 * time.Millisecond
		}
		select {
		case line, ok := <-lines:
			if !ok {
				return errors.New("app-server closed stdout")
			}
			received++
			for id := 1; id <= 3; id++ {
				marker := fmt.Sprintf("\"id\":%d", id)
				if strings.Contains(line, marker) {
					if strings.Contains(line, "\"error\"") {
						return fmt.Errorf("id %d errored", id)
					}
					answered[id] = true
					gateLine("reply", fmt.Sprintf("id %d answered", id))
				}
			}
			if !strings.Contains(line, "\"id\"") {
				notifications++
			}
		case <-time.After(left):
		}
	}
	gateLine("interaction", fmt.Sprintf(
		"config/read + thread/list answered; %d frames, %d unsolicited notifications",
		received, notifications))

	// Clean teardown: close stdin, allow graceful exit, terminate on timeout.
	_ = stdin.Close()
	waitErr := make(chan error, 1)
	go func() { waitErr <- cmd.Wait() }()
	select {
	case err := <-waitErr:
		gateLine("teardown", fmt.Sprintf("exited (%v)", err))
	case <-time.After(5 * time.Second):
		_ = cmd.Process.Kill()
		<-waitErr
		gateLine("teardown", "terminated after stdin close timeout")
	}
	gateLine("teardown", fmt.Sprintf("stderr drained %d bytes", <-stderrDone))
	return nil
}
