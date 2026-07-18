package scripts_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestVPSCreateIndexesUsesUFHotPathNames(t *testing.T) {
	root := findRepoRoot(t)
	sql, err := os.ReadFile(filepath.Join(root, "scripts/vps_create_indexes.sql"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(sql)
	if !strings.Contains(text, "idx_estab_uf_cnpj_completo") {
		t.Fatal("expected UF-partition hot-path index name idx_estab_uf_cnpj_completo")
	}
	if strings.Contains(text, "CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnpj_completo ON estabelecimentos") {
		t.Fatal("legacy global name collides with estabelecimentos_legacy_range; use idx_estab_uf_*")
	}
}
