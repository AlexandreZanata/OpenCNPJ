package perfvalidation

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPhase1DeployFilesNonEmpty(t *testing.T) {
	if len(Phase1DeployFiles) < 4 {
		t.Fatalf("deploy files = %d, want >= 4", len(Phase1DeployFiles))
	}
}

func TestPhase1SysctlExpectations(t *testing.T) {
	if len(Phase1SysctlExpectations) < 5 {
		t.Fatalf("sysctl keys = %d, want >= 5", len(Phase1SysctlExpectations))
	}
	if Phase1SysctlExpectations["vm.swappiness"] != "1" {
		t.Fatal("vm.swappiness must be 1 for DB workload")
	}
}

func TestPhase1SysctlTemplateHasRequiredKeys(t *testing.T) {
	root := findRepoRoot(t)
	path := filepath.Join(root, "deploy/vps/sysctl-opencnpj.conf.example")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read sysctl template: %v", err)
	}
	text := string(body)
	for key, want := range Phase1SysctlExpectations {
		if !strings.Contains(text, key) {
			t.Fatalf("sysctl template missing key %q", key)
		}
		if !strings.Contains(text, want) {
			t.Fatalf("sysctl template missing value %q for %q", want, key)
		}
	}
	for _, forbidden := range Phase1SysctlForbiddenSubstrings {
		if strings.Contains(strings.ToLower(text), forbidden+"=") {
			t.Fatalf("sysctl template must not contain %q assignment", forbidden)
		}
	}
}

func TestPhase1LimitsTemplate(t *testing.T) {
	root := findRepoRoot(t)
	path := filepath.Join(root, "deploy/vps/limits-postgres.conf.example")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read limits template: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, "nofile 65536") || !strings.Contains(text, "nproc 65536") {
		t.Fatal("limits template must set nofile and nproc to 65536")
	}
}

func findRepoRoot(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("go.mod not found")
		}
		dir = parent
	}
}
