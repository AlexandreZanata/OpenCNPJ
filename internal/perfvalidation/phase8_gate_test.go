package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase8DocFilesExist(t *testing.T) {
	root := findRepoRoot(t)
	for _, rel := range Phase8DocFiles {
		path := repoPath(root, rel)
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("missing %s: %v", rel, err)
		}
	}
}

func TestPhase8OpenAPIHasCnpjRoute(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, Phase8OpenAPIFile)
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read openapi: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, "/api/v1/cnpj/{cnpj}") {
		t.Fatal("openapi missing cnpj route")
	}
	if !strings.Contains(text, "ApiKeyAuth") {
		t.Fatal("openapi missing ApiKeyAuth")
	}
}

func TestPhase8GateScriptExists(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, Phase8GateScript)
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("gate script: %v", err)
	}
}
