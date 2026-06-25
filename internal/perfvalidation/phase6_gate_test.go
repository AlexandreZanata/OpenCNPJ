package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase6MigrationDefinesListUF(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "migrations", "000014_uf_list_partitions.up.sql")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, Phase6PartitionStrategy) {
		t.Fatalf("migration missing %q", Phase6PartitionStrategy)
	}
	if !strings.Contains(text, "estabelecimentos_default") {
		t.Fatal("migration must define DEFAULT uf partition")
	}
}

func TestPhase6ExplainScriptExists(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "scripts", "explain_uf_partition_pruning.sql")
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("explain script: %v", err)
	}
}

func TestPhase6BrazilianUFsInMigration(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "migrations", "000014_uf_list_partitions.up.sql")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	text := string(body)
	for _, uf := range []string{"SP", "RJ", "MG"} {
		if !strings.Contains(text, uf) {
			t.Fatalf("migration missing uf partition seed %s", uf)
		}
	}
}
