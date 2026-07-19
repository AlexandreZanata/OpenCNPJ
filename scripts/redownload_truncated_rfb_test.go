package scripts_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestRedownloadTruncatedRFBScriptSyntax(t *testing.T) {
	root := findRepoRoot(t)
	script := filepath.Join(root, "scripts", "redownload_truncated_rfb.sh")
	cmd := exec.Command("bash", "-n", script)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("bash -n: %v\n%s", err, out)
	}
}

func TestRedownloadTruncatedRFBTargets(t *testing.T) {
	root := findRepoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "scripts", "redownload_truncated_rfb.sh"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, needle := range []string{
		"Estabelecimentos0.zip",
		"Socios0.zip",
		"Simples.zip",
		"512 * 1024 * 1024",
		"cmd/downloader",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("missing %q", needle)
		}
	}
}
