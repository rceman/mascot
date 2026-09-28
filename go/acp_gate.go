package main

// Devin ACP compatibility gate (macOS Stage B). Launches `devin acp` over
// stdio with an explicit environment, completes the ACP initialize
// handshake, opens a session, issues one read-only prompt, records streamed
// session/update notifications, and tears the process down cleanly.

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

func acpLine(step, detail string) {
	clean := strings.Map(func(r rune) rune {
		if r == '"' || r == '\\' || r < ' ' {
			return ' '
		}
		return r
	}, detail)
	fmt.Printf("{\"event\":\"acp_gate\",\"step\":\"%s\",\"detail\":\"%s\"}\n", step, clean)
}

func acpGate(acpExe string) error {
	cwd := filepath.Dir(acpExe)
	home := os.Getenv("HOME")
	cmd := exec.Command(acpExe, "acp")
	cmd.Dir = cwd
	cmd.Env = []string{
		"PATH=/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin",
		"HOME=" + home,
		"LANG=en_US.UTF-8",
	}
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return fmt.Errorf("stdin pipe: %w", err)
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("stdout pipe: %w", err)
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return fmt.Errorf("stderr pipe: %w", err)
	}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("spawn: %w", err)
	}
	acpLine("spawn", "devin acp started")
	stderrDone := make(chan int64, 1)
	go func() {
		n, _ := io.Copy(io.Discard, stderr)
		stderrDone <- n
	}()
	lines := make(chan string, 64)
	go func() {
		reader := bufio.NewReader(stdout)
		for {
			line, rerr := reader.ReadString('\n')
			if len(line) > 0 {
				lines <- strings.TrimRight(line, "\r\n")
			}
			if rerr != nil {
				close(lines)
				return
			}
		}
	}()

	send := func(frame string) error {
		_, err := stdin.Write([]byte(frame + "\n"))
		if err != nil {
			return fmt.Errorf("write: %w", err)
		}
		return nil
	}

	deadline := time.Now().Add(60 * time.Second)
	got := map[int]bool{}
	updates := 0
	notifications := 0
	sessionID := ""
	if err := send(`{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},"clientInfo":{"name":"mascot-acp-gate","version":"0.1"}}}`); err != nil {
		return fmt.Errorf("initialize send: %w", err)
	}
	if err := send(`{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp","mcpServers":[]}}`); err != nil {
		return fmt.Errorf("session/new send: %w", err)
	}
	for !got[1] || !got[2] || !got[3] {
		left := time.Until(deadline)
		if left <= 0 {
			return errors.New("timeout waiting for ACP replies")
		}
		select {
		case line, open := <-lines:
			if !open {
				return errors.New("acp server closed stdout")
			}
			if strings.Contains(line, `"sessionUpdate"`) {
				updates++
			}
			if !strings.Contains(line, `"id"`) {
				notifications++
			}
			for id := 1; id <= 3; id++ {
				if strings.Contains(line, fmt.Sprintf(`"id":%d`, id)) {
					if strings.Contains(line, `"error"`) {
						return fmt.Errorf("id %d errored: %s", id, line)
					}
					if !got[id] {
						got[id] = true
						acpLine("reply", fmt.Sprintf("id %d answered", id))
					}
				}
			}
			if got[2] && sessionID == "" {
				if i := strings.Index(line, `"sessionId":"`); i >= 0 {
					rest := line[i+13:]
					if j := strings.IndexByte(rest, '"'); j >= 0 {
						sessionID = rest[:j]
						acpLine("session", "sessionId "+sessionID)
						prompt := fmt.Sprintf(`{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"%s","prompt":[{"type":"text","text":"Reply with exactly: ok"}]}}`, sessionID)
						if err := send(prompt); err != nil {
							return fmt.Errorf("prompt send: %w", err)
						}
					}
				}
			}
		case <-time.After(left):
			return errors.New("timeout waiting for ACP replies")
		}
	}
	acpLine("interaction", fmt.Sprintf("initialize + session/new + session/prompt answered; %d session/update notifications", updates))

	stdin.Close()
	waitDone := make(chan error, 1)
	go func() { waitDone <- cmd.Wait() }()
	select {
	case werr := <-waitDone:
		acpLine("teardown", fmt.Sprintf("exited err=%v", werr))
	case <-time.After(5 * time.Second):
		_ = cmd.Process.Kill()
		<-waitDone
		acpLine("teardown", "terminated after stdin close timeout")
	}
	stderrBytes := <-stderrDone
	acpLine("teardown", fmt.Sprintf("stderr drained %d bytes", stderrBytes))
	return nil
}
