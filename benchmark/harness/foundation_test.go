package harness

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestClockAndMissingMetrics(t *testing.T) {
	if Frequency() <= 0 || QPC() > QPC() {
		t.Fatal("invalid common clock")
	}
	data, err := json.Marshal(ResourceSample{Unavailable: map[string]string{"private_working_set_bytes": "not sampled"}})
	if err != nil || !strings.Contains(string(data), `"private_working_set_bytes":null`) {
		t.Fatal("missing metrics must be null, not invented zeros", err)
	}
}

func TestImmutableRunDirectory(t *testing.T) {
	root := t.TempDir()
	if _, err := NewRunDirectory(root, "run-001"); err != nil {
		t.Fatal(err)
	}
	for _, id := range []string{"run-001", "", "..", `..\escape`, "../escape"} {
		if _, err := NewRunDirectory(root, id); err == nil {
			t.Fatalf("accepted reused/invalid run ID %q", id)
		}
	}
}

func TestLaunchSanitized(t *testing.T) {
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("MASCOT_PARENT_ONLY", "must-not-be-inherited")
	process, err := Launch(LaunchSpec{
		Executable: executable, Arguments: []string{"-test.run=^TestHarnessChild$"},
		WorkingDirectory: filepath.Dir(executable),
		Environment:      map[string]string{"SystemRoot": os.Getenv("SystemRoot"), "MASCOT_HARNESS_CHILD": "1"},
		Role:             "harness-self-test", Counted: false,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer process.Command.Process.Kill()
	out := make(chan []byte, 1)
	errout := make(chan []byte, 1)
	go func() { data, _ := io.ReadAll(process.Stdout); out <- data }()
	go func() { data, _ := io.ReadAll(process.Stderr); errout <- data }()
	output, diagnostic := <-out, <-errout
	if err := process.Command.Wait(); err != nil {
		t.Fatal(err)
	}
	if string(output) != "ready" || string(diagnostic) != "diagnostic" || process.PID <= 0 || process.Launch > QPC() {
		t.Fatalf("unexpected native launch result %q %q", output, diagnostic)
	}
}

func TestHarnessChild(t *testing.T) {
	if os.Getenv("MASCOT_HARNESS_CHILD") != "1" {
		return
	}
	if os.Getenv("MASCOT_PARENT_ONLY") != "" {
		os.Exit(10)
	}
	fmt.Fprint(os.Stdout, "ready")
	fmt.Fprint(os.Stderr, "diagnostic")
	os.Exit(0)
}
