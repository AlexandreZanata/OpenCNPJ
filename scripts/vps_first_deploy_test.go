package scripts_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestVpsFirstDeploySwapUsesSeparateDropDatabase(t *testing.T) {
	root := findRepoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "scripts", "vps_first_deploy.sh"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	if strings.Contains(text, `DROP DATABASE IF EXISTS opencnpj_cnpj;
    ALTER DATABASE`) {
		t.Fatal("DROP DATABASE must not share a psql -c block with other statements")
	}
	for _, needle := range []string{
		`DROP DATABASE IF EXISTS opencnpj_cnpj;"`,
		`ALTER DATABASE ${staging} RENAME TO opencnpj_cnpj;"`,
		"ALTER ROLE opencnpj_reader PASSWORD",
		"SKIP_ANALYZE=1 create_search_indexes",
		"Reference tables already loaded",
	} {
		if !strings.Contains(text, needle) {
			t.Fatalf("missing %q", needle)
		}
	}
}
