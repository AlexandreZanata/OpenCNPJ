package scripts_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestRepairTruncatedPipelineSyntax(t *testing.T) {
	root := findRepoRoot(t)
	script := filepath.Join(root, "scripts", "repair_truncated_pipeline.sh")
	cmd := exec.Command("bash", "-n", script)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("bash -n: %v\n%s", err, out)
	}
}

func TestRepairTruncatedPipelineSteps(t *testing.T) {
	root := findRepoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "scripts", "repair_truncated_pipeline.sh"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, needle := range []string{
		"run_full_import.sh",
		"pc_to_vps_sync.sh",
		"RESTORE_ONLY=1",
		"assert_no_truncated_csvs",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("missing %q", needle)
		}
	}
}

func TestVpsFirstDeployRestoreOnly(t *testing.T) {
	root := findRepoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "scripts", "vps_first_deploy.sh"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), `RESTORE_ONLY="${RESTORE_ONLY:-0}"`) {
		t.Fatal("missing RESTORE_ONLY flag")
	}
}
