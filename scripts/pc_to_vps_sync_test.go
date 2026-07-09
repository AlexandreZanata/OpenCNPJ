package scripts_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestPcToVpsSyncScriptSyntax(t *testing.T) {
	root := findRepoRoot(t)
	script := filepath.Join(root, "scripts", "pc_to_vps_sync.sh")
	data, err := os.ReadFile(script)
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, needle := range []string{
		"wait_for_import",
		"local_dump",
		"upload_to_vps",
		"VPS_HOST",
		"pg_dump",
		"rsync",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("missing %q in pc_to_vps_sync.sh", needle)
		}
	}
	cmd := exec.Command("bash", "-n", script)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("bash -n failed: %v\n%s", err, out)
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
