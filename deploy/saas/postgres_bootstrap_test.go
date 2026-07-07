package saas_test

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func deployPath(t *testing.T, name string) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	return filepath.Join(filepath.Dir(file), name)
}

func TestPostgresBootstrapTemplate(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "postgres-bootstrap.sql.example"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	required := []string{
		"opencnpj_cnpj",
		"opencnpj_saas",
		"opencnpj_reader",
		"opencnpj_restore",
		"GRANT SELECT ON ALL TABLES",
		"opencnpj_migrate_saas",
	}
	for _, needle := range required {
		if !strings.Contains(content, needle) {
			t.Errorf("bootstrap SQL missing %q", needle)
		}
	}
}

func TestAPIEnvTemplate(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "api.env.example"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	for _, needle := range []string{
		"CNPJ_DATABASE_URL=",
		"SAAS_DATABASE_URL=",
		"REDIS_URL=",
		"CONFIG_FILE=",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("api.env missing %q", needle)
		}
	}
}

func TestPgBouncerTemplate(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "pgbouncer.ini.example"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	if !strings.Contains(content, "opencnpj_cnpj") || !strings.Contains(content, "opencnpj_saas") {
		t.Fatal("pgbouncer template must list both logical databases")
	}
}
