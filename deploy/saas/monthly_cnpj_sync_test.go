package saas_test

import (
	"os"
	"os/exec"
	"strings"
	"testing"
)

func TestMonthlyCNPJSyncScriptCommands(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "monthly-cnpj-sync.example.sh"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	for _, needle := range []string{
		"local-dump",
		"upload",
		"vps-restore",
		"vps-rollback",
		"vps-drop-old",
		"pg_dump -Fc",
		"pg_restore",
		"opencnpj_cnpj_new",
		"opencnpj_cnpj_old",
		"grant-reader.sql",
		"cnpj:*",
		"opencnpj_saas",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("monthly sync script missing %q", needle)
		}
	}
}

func TestMonthlyCNPJSyncScriptSyntax(t *testing.T) {
	script := deployPath(t, "monthly-cnpj-sync.example.sh")
	cmd := exec.Command("bash", "-n", script)
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("bash -n failed: %v\n%s", err, out)
	}
}

func TestGrantReaderSQLTemplate(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "grant-reader.sql.example"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	for _, needle := range []string{
		"opencnpj_reader",
		"GRANT SELECT ON ALL TABLES",
		"ALTER DEFAULT PRIVILEGES",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("grant-reader SQL missing %q", needle)
		}
	}
}

func TestMonthlySyncScriptUsageWithoutArgs(t *testing.T) {
	script := deployPath(t, "monthly-cnpj-sync.example.sh")
	cmd := exec.Command("bash", script)
	out, err := cmd.CombinedOutput()
	if err == nil {
		t.Fatal("expected failure without command")
	}
	if !strings.Contains(string(out), "Usage:") {
		t.Fatalf("unexpected output: %s", out)
	}
}

func TestMonthlySyncDryRunRestorePrintsSwap(t *testing.T) {
	script := deployPath(t, "monthly-cnpj-sync.example.sh")
	cmd := exec.Command("bash", script, "vps-restore")
	cmd.Env = append(os.Environ(),
		"DRY_RUN=1",
		"INCOMING_DIR=/tmp/opencnpj-incoming-test",
		"DUMP_TAG=2099-01",
	)
	out, err := cmd.CombinedOutput()
	if err == nil {
		t.Fatalf("expected missing dump failure, got: %s", out)
	}
	if !strings.Contains(string(out), "Missing archive") {
		t.Fatalf("unexpected output: %s", out)
	}
}
