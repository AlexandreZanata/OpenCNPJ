package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase12RequiredFilesExist(t *testing.T) {
	root := findRepoRoot(t)
	for _, rel := range Phase12RequiredFiles {
		if _, err := os.Stat(repoPath(root, rel)); err != nil {
			t.Fatalf("missing %s: %v", rel, err)
		}
	}
}

func TestPhase12DataAccessDocCoversStack(t *testing.T) {
	root := findRepoRoot(t)
	body, err := os.ReadFile(repoPath(root, Phase12DataAccessDoc))
	if err != nil {
		t.Fatal(err)
	}
	text := string(body)
	for _, needle := range []string{
		"sqlc",
		"pgx v5",
		"errgroup",
		"idx_estabelecimentos_cnpj_completo",
		"idx_api_keys_hash",
		"saas_data_access_gate.sh",
		"lib/pq",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("data-access doc missing %q", needle)
		}
	}
}

func TestPhase12CNPJServiceRespectsGoroutineBudget(t *testing.T) {
	root := findRepoRoot(t)
	body, err := os.ReadFile(repoPath(root, "internal/cnpj/service.go"))
	if err != nil {
		t.Fatal(err)
	}
	count := strings.Count(string(body), "g.Go(")
	if count != MaxCNPJLookupFanOut {
		t.Fatalf("fetchParallel fan-out = %d, want %d", count, MaxCNPJLookupFanOut)
	}
}

func TestPhase12GateScriptExists(t *testing.T) {
	root := findRepoRoot(t)
	info, err := os.Stat(repoPath(root, Phase12GateScript))
	if err != nil {
		t.Fatalf("gate script: %v", err)
	}
	if info.Mode()&0o111 == 0 {
		t.Fatal("saas_data_access_gate.sh should be executable")
	}
}

func TestPhase12CNPJSchemaHasCompletoIndex(t *testing.T) {
	root := findRepoRoot(t)
	body, err := os.ReadFile(repoPath(root, "db/schema/cnpj.sql"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(body), Phase12CNPJIndex) {
		t.Fatalf("schema missing %s", Phase12CNPJIndex)
	}
}

func TestPhase12VPSIndexScriptAvoidsLegacyNameCollision(t *testing.T) {
	root := findRepoRoot(t)
	body, err := os.ReadFile(repoPath(root, Phase12VPSIndexScript))
	if err != nil {
		t.Fatal(err)
	}
	text := string(body)
	if !strings.Contains(text, Phase12VPSCNPJIndex) {
		t.Fatalf("VPS index script missing hot-path index %s", Phase12VPSCNPJIndex)
	}
	if !strings.Contains(text, "idx_estab_uf_cnpj_basico") {
		t.Fatal("VPS index script missing idx_estab_uf_cnpj_basico")
	}
	// Index names are schema-global. IF NOT EXISTS with the legacy name skips when
	// estabelecimentos_legacy_range still owns idx_estabelecimentos_cnpj_completo,
	// leaving UF partitions without indexes (multi-second seq scans on lookup).
	legacyCreate := "CREATE INDEX IF NOT EXISTS " + Phase12CNPJIndex + " ON estabelecimentos"
	if strings.Contains(text, legacyCreate) {
		t.Fatalf("VPS script must not use colliding name %s on estabelecimentos; use %s",
			Phase12CNPJIndex, Phase12VPSCNPJIndex)
	}
}

func TestPhase12DataAccessDocCoversVPSHotPathIndex(t *testing.T) {
	root := findRepoRoot(t)
	body, err := os.ReadFile(repoPath(root, Phase12DataAccessDoc))
	if err != nil {
		t.Fatal(err)
	}
	text := string(body)
	for _, needle := range []string{Phase12VPSCNPJIndex, Phase12VPSIndexScript, "legacy"} {
		if !strings.Contains(text, needle) {
			t.Fatalf("data-access doc missing %q", needle)
		}
	}
}

func TestPhase12SaaSMigrationsDefineIndexes(t *testing.T) {
	root := findRepoRoot(t)
	meta, err := os.ReadFile(repoPath(root, "migrations/saas/000001_saas_metadata.up.sql"))
	if err != nil {
		t.Fatal(err)
	}
	idx, err := os.ReadFile(repoPath(root, "migrations/saas/000002_saas_indexes.up.sql"))
	if err != nil {
		t.Fatal(err)
	}
	combined := string(meta) + string(idx)
	for _, needle := range Phase12SaaSIndexes {
		if !strings.Contains(combined, needle) {
			t.Fatalf("saas migrations missing index reference %q", needle)
		}
	}
}
