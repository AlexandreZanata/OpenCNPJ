package perfvalidation

import (
	"os"
	"strings"
	"testing"
)

func TestPhase4MaterializedViewsList(t *testing.T) {
	if len(Phase4MaterializedViews) != 5 {
		t.Fatalf("mv count = %d, want 5", len(Phase4MaterializedViews))
	}
}

func TestPhase4MigrationDefinesMVs(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "migrations", "000013_materialized_views.up.sql")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	text := string(body)
	for _, mv := range Phase4MaterializedViews {
		if !strings.Contains(text, mv) {
			t.Fatalf("migration missing %s", mv)
		}
	}
	if !strings.Contains(text, "REFRESH MATERIALIZED VIEW CONCURRENTLY") {
		t.Fatal("migration must use CONCURRENTLY refresh")
	}
	if !strings.Contains(text, Phase4RefreshFunction) {
		t.Fatal("migration must define refresh function")
	}
}

func TestPhase4RepositoryUsesMVNames(t *testing.T) {
	root := findRepoRoot(t)
	path := repoPath(root, "internal", "repository", "stats_repo.go")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read stats_repo: %v", err)
	}
	if !strings.Contains(string(body), "mv_stats_estabelecimentos_by_uf") {
		t.Fatal("stats_repo must query materialized views")
	}
}
