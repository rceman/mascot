//go:build windows

package harness

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"syscall"
	"unsafe"
)

var kernel32 = syscall.NewLazyDLL("kernel32.dll")
var counter = kernel32.NewProc("QueryPerformanceCounter")
var frequency = kernel32.NewProc("QueryPerformanceFrequency")

func QPC() int64 {
	var value int64
	ok, _, _ := counter.Call(uintptr(unsafe.Pointer(&value)))
	if ok == 0 {
		panic("QueryPerformanceCounter failed")
	}
	return value
}

func Frequency() int64 {
	var value int64
	ok, _, _ := frequency.Call(uintptr(unsafe.Pointer(&value)))
	if ok == 0 || value <= 0 {
		panic("QueryPerformanceFrequency failed")
	}
	return value
}

type LaunchSpec struct {
	Executable       string            `json:"executable"`
	Arguments        []string          `json:"arguments"`
	WorkingDirectory string            `json:"working_directory"`
	Environment      map[string]string `json:"environment"`
	Role             string            `json:"role"`
	Counted          bool              `json:"counted_in_application_total"`
}

type Process struct {
	Command *exec.Cmd
	Stdin   io.WriteCloser
	Stdout  io.ReadCloser
	Stderr  io.ReadCloser
	Launch  int64
	PID     int
	Spec    LaunchSpec
}

func Launch(spec LaunchSpec) (*Process, error) {
	if !filepath.IsAbs(spec.Executable) || !filepath.IsAbs(spec.WorkingDirectory) {
		return nil, errors.New("launch paths must be absolute and frozen")
	}
	if spec.Environment["SystemRoot"] == "" {
		return nil, errors.New("explicit sanitized Windows environment required")
	}
	command := exec.Command(spec.Executable, spec.Arguments...)
	command.Dir = spec.WorkingDirectory
	keys := make([]string, 0, len(spec.Environment))
	for key := range spec.Environment {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	command.Env = make([]string, 0, len(keys))
	for _, key := range keys {
		command.Env = append(command.Env, key+"="+spec.Environment[key])
	}
	process := &Process{Command: command, Spec: spec}
	var err error
	if process.Stdin, err = command.StdinPipe(); err != nil {
		return nil, err
	}
	if process.Stdout, err = command.StdoutPipe(); err != nil {
		process.Stdin.Close()
		return nil, err
	}
	if process.Stderr, err = command.StderrPipe(); err != nil {
		process.Stdin.Close()
		process.Stdout.Close()
		return nil, err
	}
	process.Launch = QPC()
	if err := command.Start(); err != nil {
		process.Stdin.Close()
		process.Stdout.Close()
		process.Stderr.Close()
		return nil, err
	}
	process.PID = command.Process.Pid
	return process, nil
}

type Event struct {
	RunID           string          `json:"run_id"`
	FixtureVersion  string          `json:"fixture_version"`
	Candidate       string          `json:"candidate"`
	BuildSHA256     string          `json:"candidate_build_sha256"`
	ConfigurationID string          `json:"configuration_id"`
	ProcessLifetime int             `json:"process_lifetime"`
	Scenario        string          `json:"scenario"`
	QPC             int64           `json:"qpc"`
	Frequency       int64           `json:"qpc_frequency"`
	Kind            string          `json:"kind"`
	RequestID       *int64          `json:"request_id"`
	Sequence        *int            `json:"seq"`
	Details         json.RawMessage `json:"details"`
}

type Inventory struct {
	PID             int    `json:"pid"`
	ParentPID       int    `json:"parent_pid"`
	Role            string `json:"role"`
	CreationQPC     int64  `json:"creation_qpc"`
	ExitQPC         *int64 `json:"exit_qpc"`
	Counted         bool   `json:"counted"`
	ExclusionReason string `json:"exclusion_reason"`
}

type ResourceSample struct {
	QPC               int64             `json:"qpc"`
	PID               int               `json:"pid"`
	PrivateWorkingSet *uint64           `json:"private_working_set_bytes"`
	PrivateCommit     *uint64           `json:"private_commit_bytes"`
	WorkingSet        *uint64           `json:"working_set_bytes"`
	UserCPU           *uint64           `json:"user_cpu_100ns"`
	KernelCPU         *uint64           `json:"kernel_cpu_100ns"`
	Threads           *uint32           `json:"threads"`
	KernelHandles     *uint32           `json:"kernel_handles"`
	USERObjects       *uint32           `json:"user_objects"`
	GDIObjects        *uint32           `json:"gdi_objects"`
	LiveWindows       *uint32           `json:"live_windows"`
	LiveChildren      *uint32           `json:"live_children"`
	Redraws           *uint64           `json:"redraws"`
	Presents          *uint64           `json:"presents"`
	QueueDepth        *uint32           `json:"bounded_queue_depth"`
	Unavailable       map[string]string `json:"unavailable_reasons"`
}

func NewRunDirectory(root, runID string) (string, error) {
	if runID == "" || filepath.Base(runID) != runID || runID == "." || runID == ".." {
		return "", errors.New("run ID must be one nonempty path component")
	}
	if err := os.MkdirAll(root, 0755); err != nil {
		return "", err
	}
	path := filepath.Join(root, runID)
	if err := os.Mkdir(path, 0755); err != nil {
		return "", err
	}
	return path, nil
}
