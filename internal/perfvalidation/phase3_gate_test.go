package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase3RequiredMetrics(t *testing.T) {
	if len(Phase3RequiredMetrics) != 2 {
		t.Fatalf("metrics = %d, want 2", len(Phase3RequiredMetrics))
	}
}

func TestPhase3ConfigYAML(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "config", "config.yaml")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}
	text := string(body)
	for _, key := range Phase3ConfigKeys {
		if !strings.Contains(text, key) {
			t.Fatalf("config.yaml missing cache.%s", key)
		}
	}
	if !strings.Contains(text, "l1_enabled: true") {
		t.Fatal("expected l1_enabled: true in config.yaml")
	}
}

func TestPhase3L1PackageExists(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "internal", "cache", "l1", "cache.go")
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("missing L1 package: %v", err)
	}
}
