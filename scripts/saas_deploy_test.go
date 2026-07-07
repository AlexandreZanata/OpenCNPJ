package scripts_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	return filepath.Join(filepath.Dir(file), "..")
}

func TestSaasSmokeScriptRejectsMissingURL(t *testing.T) {
	root := repoRoot(t)
	script := filepath.Join(root, "scripts", "saas_smoke.sh")
	cmd := exec.Command("bash", script)
	out, err := cmd.CombinedOutput()
	if err == nil {
		t.Fatal("expected failure without BASE_URL")
	}
	if !strings.Contains(string(out), "BASE_URL") {
		t.Fatalf("unexpected output: %s", out)
	}
}

func TestBuildScriptProducesBinary(t *testing.T) {
	root := repoRoot(t)
	tmp := t.TempDir()
	out := filepath.Join(tmp, "opencnpj-api")
	script := filepath.Join(root, "scripts", "build_opencnpj_api.sh")
	if err := exec.Command("bash", script, out).Run(); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(out)
	if err != nil {
		t.Fatal(err)
	}
	if info.Size() < 1024 {
		t.Fatalf("binary too small: %d bytes", info.Size())
	}
}
