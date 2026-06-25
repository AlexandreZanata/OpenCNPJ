package perfvalidation

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPhase5ConfigKeys(t *testing.T) {
	if len(Phase5ConfigKeys) < 1 {
		t.Fatal("expected config keys")
	}
}

func TestPhase5SelectiveSQLInIndexer(t *testing.T) {
	root := findRepoRoot(t)
	path := filepath.Join(root, "internal/meilisearch/selective.go")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read selective.go: %v", err)
	}
	text := string(body)
	for _, marker := range Phase5SelectiveSQLMarkers {
		if !strings.Contains(text, marker) {
			t.Fatalf("selective.go missing %q", marker)
		}
	}
}

func TestPhase5MeilisearchIndexCMD(t *testing.T) {
	root := findRepoRoot(t)
	path := filepath.Join(root, "cmd/meilisearch-index/main.go")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read meilisearch-index: %v", err)
	}
	if !strings.Contains(string(body), "SelectiveActiveMatriz") {
		t.Fatal("meilisearch-index must pass selective option")
	}
}
