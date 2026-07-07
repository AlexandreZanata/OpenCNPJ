package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase10DeployArtifactsExist(t *testing.T) {
	root := findRepoRoot(t)
	for _, rel := range Phase10DeployArtifacts {
		path := repoPath(root, rel)
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("missing %s: %v", rel, err)
		}
	}
}

func TestPhase10RunbookHasSmokeStep(t *testing.T) {
	root := findRepoRoot(t)
	body, err := os.ReadFile(repoPath(root, Phase10DeployRunbook))
	if err != nil {
		t.Fatal(err)
	}
	text := string(body)
	for _, needle := range []string{
		"saas_smoke.sh",
		"systemd-opencnpj-api.example",
		"admin-bootstrap",
		"rollback.example.sh",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("runbook missing %q", needle)
		}
	}
}

func TestPhase10GateScriptExists(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, Phase10GateScript)
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("gate script: %v", err)
	}
	if info.Mode()&0o111 == 0 {
		t.Fatal("saas_deploy_gate.sh should be executable")
	}
}
