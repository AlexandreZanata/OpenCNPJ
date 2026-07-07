package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase9SecurityFilesExist(t *testing.T) {
	root := findRepoRoot(t)
	for _, rel := range Phase9CodeMarkers {
		path := repoPath(root, rel)
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("missing %s: %v", rel, err)
		}
	}
}

func TestPhase9SecurityDocHasSaaSSection(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, Phase9SecurityDoc)
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read security doc: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, "## 8. SaaS production hardening") {
		t.Fatal("SECURITY.md missing SaaS hardening section")
	}
	if !strings.Contains(text, "X-API-Key") {
		t.Fatal("SECURITY.md missing API key guidance")
	}
}

func TestPhase9GateScriptExists(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, Phase9GateScript)
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("gate script: %v", err)
	}
}
