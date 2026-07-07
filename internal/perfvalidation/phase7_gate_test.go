package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase7MigrationDefinesCNAEHashSubPartitions(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "migrations", "000016_cnae_hash_subpartitions.up.sql")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, Phase7TopPartitionStrategy) {
		t.Fatalf("migration missing %q", Phase7TopPartitionStrategy)
	}
	if !strings.Contains(text, Phase7SubPartitionStrategy) {
		t.Fatalf("migration missing %q", Phase7SubPartitionStrategy)
	}
	if !strings.Contains(text, "estabelecimentos_default_h") {
		t.Fatal("migration must define DEFAULT CNAE hash sub-partitions")
	}
}

func TestPhase7ExplainScriptExists(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "scripts", "explain_cnae_uf_partition_pruning.sql")
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("explain script: %v", err)
	}
}

func TestPhase7CNAEHashBucketsInMigration(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "migrations", "000016_cnae_hash_subpartitions.up.sql")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	text := string(body)
	if !strings.Contains(text, "MODULUS 4") {
		t.Fatal("migration must use MODULUS 4 hash buckets per UF")
	}
}
