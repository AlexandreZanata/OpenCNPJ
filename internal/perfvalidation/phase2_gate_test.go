package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase2DeployFilesNonEmpty(t *testing.T) {
	if len(Phase2DeployFiles) < 4 {
		t.Fatalf("deploy files = %d, want >= 4", len(Phase2DeployFiles))
	}
}

func TestPhase2GUCExpectations(t *testing.T) {
	if Phase2GUCExpectations["shared_buffers"] != "4GB" {
		t.Fatal("shared_buffers must be 4GB on 16 GB VPS")
	}
	if Phase2GUCExpectations["work_mem"] != "64MB" {
		t.Fatal("work_mem must be 64MB with pgBouncer transaction pooling")
	}
}

func TestPhase2PostgreSQLTemplateHasRequiredGUCs(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "deploy", "vps", "postgresql-opencnpj.conf.example")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read postgresql template: %v", err)
	}
	text := string(body)
	for key, want := range Phase2GUCExpectations {
		if key == "autovacuum" {
			continue // in autovacuum include file
		}
		if !strings.Contains(text, key+" = "+want) {
			t.Fatalf("postgresql template missing %s = %s", key, want)
		}
	}
	autoPath := repoPath(root, "deploy", "vps", "postgresql-autovacuum-opencnpj.conf.example")
	autoBody, err := os.ReadFile(autoPath)
	if err != nil {
		t.Fatalf("read autovacuum template: %v", err)
	}
	if !strings.Contains(string(autoBody), "autovacuum = on") {
		t.Fatal("autovacuum template must enable autovacuum")
	}
	for _, forbidden := range Phase2ForbiddenGUCAssignments {
		if strings.Contains(strings.ToLower(text), forbidden) {
			t.Fatalf("postgresql template must not contain %q", forbidden)
		}
	}
}

func TestPhase2AnalyzeSQL(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "deploy", "vps", "analyze-search-tables.sql.example")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read analyze sql: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, "ANALYZE empresas") || !strings.Contains(text, "ANALYZE estabelecimentos") {
		t.Fatal("analyze SQL must ANALYZE empresas and estabelecimentos")
	}
	if !strings.Contains(text, "opencnpj_set_partition_autovacuum") {
		t.Fatal("analyze SQL must set autovacuum on partition children")
	}
}
