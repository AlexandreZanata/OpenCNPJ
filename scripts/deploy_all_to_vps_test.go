package scripts_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestDeployAllToVpsScriptSyntax(t *testing.T) {
	root := findRepoRoot(t)
	for _, name := range []string{"deploy_all_to_vps.sh", "vps_first_deploy.sh"} {
		script := filepath.Join(root, "scripts", name)
		cmd := exec.Command("bash", "-n", script)
		if out, err := cmd.CombinedOutput(); err != nil {
			t.Fatalf("%s: %v\n%s", name, err, out)
		}
	}
}

func TestDeployAllScriptContainsSteps(t *testing.T) {
	root := findRepoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "scripts", "deploy_all_to_vps.sh"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, needle := range []string{
		"pg_dump", "opencnpj-api", "vps_first_deploy.sh",
		"credentials.txt", "migrations/", "API_ONLY=1",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("missing %q", needle)
		}
	}
}
