package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"
	"unsafe"
)

const fixtureVersion = "windows-v1.0.2"
const frameLimit = 65536
const cancelTimeout = time.Second

var kernel32 = syscall.NewLazyDLL("kernel32.dll")
var queryCounter = kernel32.NewProc("QueryPerformanceCounter")
var queryFrequency = kernel32.NewProc("QueryPerformanceFrequency")
var shutdown = errors.New("shutdown")

func qpc() int64 {
	var value int64
	ok, _, _ := queryCounter.Call(uintptr(unsafe.Pointer(&value)))
	if ok == 0 {
		panic("QueryPerformanceCounter failed")
	}
	return value
}

func frequency() int64 {
	var value int64
	ok, _, _ := queryFrequency.Call(uintptr(unsafe.Pointer(&value)))
	if ok == 0 || value <= 0 {
		panic("QueryPerformanceFrequency failed")
	}
	return value
}

type command struct {
	Type      string `json:"type"`
	ID        int64  `json:"id"`
	Prompt    string `json:"prompt"`
	Scenario  string `json:"scenario"`
	RequestID int64  `json:"request_id"`
	Result    struct {
		Accepted bool `json:"accepted"`
	} `json:"result"`
}

type received struct {
	Command command
	Err     error
}

func readCommands(input io.Reader, output chan<- received) {
	defer close(output)
	reader := bufio.NewReaderSize(input, frameLimit)
	for {
		line, err := reader.ReadSlice('\n')
		if err != nil {
			if errors.Is(err, io.EOF) && len(line) == 0 {
				return
			}
			output <- received{Err: fmt.Errorf("invalid stdin frame: %w", err)}
			return
		}
		var value command
		if err = json.Unmarshal(line, &value); err != nil {
			output <- received{Err: err}
			return
		}
		output <- received{Command: value}
	}
}

func wire(value map[string]any) []byte {
	data, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return append(data, '\n')
}

func writeAll(data []byte) error {
	n, err := os.Stdout.Write(data)
	if err == nil && n != len(data) {
		err = io.ErrShortWrite
	}
	return err
}

func event(kind string, id int64, fields map[string]any) error {
	if fields == nil {
		fields = make(map[string]any)
	}
	fields["type"] = kind
	if id != 0 {
		fields["id"] = id
	}
	return writeAll(wire(fields))
}

func chunk(id int64, sequence int, text string) []byte {
	return wire(map[string]any{"type": "chunk", "id": id, "seq": sequence, "text": text, "emit_qpc": strings.Repeat("0", 20)})
}

func emit(frames [][]byte, pattern string) error {
	var data []byte
	starts := make([]int, len(frames))
	stamps := make([]int, len(frames))
	marker := []byte(`"emit_qpc":"`)
	for index, frame := range frames {
		starts[index] = len(data)
		position := bytes.Index(frame, marker)
		if position < 0 {
			return errors.New("chunk timestamp field missing")
		}
		stamps[index] = len(data) + position + len(marker)
		data = append(data, frame...)
	}
	cuts := []int{len(data)}
	switch pattern {
	case "small":
		cuts = []int{1, 7, len(data)}
	case "utf8":
		for index, value := range data {
			if value >= 0xc2 {
				cuts = []int{index + 1, len(data)}
				break
			}
		}
	case "newline":
		cuts = []int{len(data) - 1, len(data)}
	}
	begin := 0
	for _, end := range cuts {
		timestamp := qpc()
		for index, start := range starts {
			if start >= begin && start < end {
				var digits [20]byte
				value := timestamp
				for digit := len(digits) - 1; digit >= 0; digit-- {
					digits[digit] = byte(value%10) + '0'
					value /= 10
				}
				copy(data[stamps[index]:stamps[index]+20], digits[:])
			}
		}
		if err := writeAll(data[begin:end]); err != nil {
			return err
		}
		begin = end
	}
	return nil
}

func activeCommand(input <-chan received, id int64, last int, wait time.Duration) (bool, error) {
	timer := time.NewTimer(wait)
	defer timer.Stop()
	select {
	case incoming, ok := <-input:
		if !ok {
			return false, io.EOF
		}
		if incoming.Err != nil {
			return false, incoming.Err
		}
		value := incoming.Command
		switch {
		case value.Type == "shutdown":
			if err := event("shutdown_ack", 0, nil); err != nil {
				return false, err
			}
			return false, shutdown
		case value.Type == "cancel" && value.ID == id:
			return true, event("cancelled", id, map[string]any{"last_seq": last})
		default:
			return false, errors.New("unexpected command during active request")
		}
	case <-timer.C:
		return false, nil
	}
}

func serve(value command, input <-chan received, payload []string) error {
	scenario := value.Scenario
	if scenario == "" {
		scenario = "normal"
	}
	count := 100
	switch scenario {
	case "normal", "fragmented", "cancel", "client_request", "stderr", "unexpected_exit":
	case "backpressure":
		count = 256
	case "maximum", "oversized":
		count = 1
	default:
		return errors.New("unknown scenario")
	}
	if err := event("start", value.ID, map[string]any{"chunks": count, "qpc_frequency": frequency()}); err != nil {
		return err
	}
	if scenario == "client_request" {
		if err := event("client_request", value.ID, map[string]any{"request_id": 1, "method": "benchmark.confirm", "params": map[string]any{"value": "ok"}}); err != nil {
			return err
		}
		select {
		case incoming, ok := <-input:
			if !ok || incoming.Err != nil || incoming.Command.Type != "client_response" || incoming.Command.RequestID != 1 || !incoming.Command.Result.Accepted {
				return errors.New("incorrect client response")
			}
		case <-time.After(cancelTimeout):
			return errors.New("client response timeout")
		}
	}
	var stderrDone <-chan error
	if scenario == "stderr" {
		done := make(chan error, 1)
		stderrDone = done
		go func() {
			block := bytes.Repeat([]byte("fixture-stderr\n"), 256*1024/15+1)
			_, err := os.Stderr.Write(block[:256*1024])
			done <- err
		}()
	}
	for sequence := 0; sequence < count; sequence++ {
		text := payload[sequence%len(payload)]
		frames := [][]byte{chunk(value.ID, sequence, text)}
		pattern := "whole"
		if scenario == "maximum" || scenario == "oversized" {
			size := frameLimit
			if scenario == "oversized" {
				size++
			}
			padding := size - len(chunk(value.ID, sequence, ""))
			frames[0] = chunk(value.ID, sequence, strings.Repeat("x", padding))
		} else if scenario == "fragmented" {
			switch sequence % 10 {
			case 1:
				pattern = "small"
			case 2:
				sequence++
				frames = append(frames, chunk(value.ID, sequence, payload[sequence%len(payload)]))
			case 4:
				pattern = "utf8"
			case 5:
				pattern = "newline"
			}
		} else if sequence == 4 {
			pattern = "utf8"
		}
		if err := emit(frames, pattern); err != nil {
			return err
		}
		if scenario == "unexpected_exit" && sequence == 6 {
			os.Exit(23)
		}
		if scenario == "oversized" {
			_, err := activeCommand(input, value.ID, sequence, 5*time.Second)
			if err != nil {
				return err
			}
			return errors.New("oversized session was not torn down")
		}
		if scenario == "cancel" && sequence == 49 {
			cancelled, err := activeCommand(input, value.ID, sequence, cancelTimeout)
			if err != nil {
				return err
			}
			if !cancelled {
				return errors.New("canonical cancellation timeout")
			}
			return nil
		}
		if scenario != "backpressure" && sequence+1 < count {
			cancelled, err := activeCommand(input, value.ID, sequence, 10*time.Millisecond)
			if cancelled || err != nil {
				return err
			}
		}
	}
	if stderrDone != nil {
		if err := <-stderrDone; err != nil {
			return err
		}
	}
	return event("complete", value.ID, nil)
}

func provider(root string) error {
	data, err := os.ReadFile(filepath.Join(root, "fixtures", "chunks.json"))
	if err != nil {
		return err
	}
	var payload []string
	if err := json.Unmarshal(data, &payload); err != nil || len(payload) != 100 {
		return errors.New("invalid payload fixture")
	}
	input := make(chan received, 16)
	go readCommands(os.Stdin, input)
	for incoming := range input {
		if incoming.Err != nil {
			return incoming.Err
		}
		value := incoming.Command
		if value.Type == "shutdown" {
			return event("shutdown_ack", 0, nil)
		}
		if value.Type != "request" || value.ID <= 0 {
			return errors.New("invalid ordinary request")
		}
		if err := serve(value, input, payload); err != nil {
			if errors.Is(err, shutdown) {
				return nil
			}
			return err
		}
	}
	return nil
}

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: fixture.exe materialize|provider|validate ROOT")
		os.Exit(64)
	}
	root, err := filepath.Abs(os.Args[2])
	if err == nil {
		switch os.Args[1] {
		case "provider":
			err = provider(root)
		case "materialize":
			err = materialize(root, false)
		case "refresh-unfrozen":
			err = materialize(root, true)
		case "validate":
			err = validate(root)
		case "environment":
			err = reportEnvironment()
		default:
			err = errors.New("unknown operation")
		}
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(64)
	}
}

func decimal(value any) (int64, error) {
	switch value := value.(type) {
	case json.Number:
		return value.Int64()
	case string:
		return strconv.ParseInt(value, 10, 64)
	default:
		return 0, errors.New("integer field missing")
	}
}
