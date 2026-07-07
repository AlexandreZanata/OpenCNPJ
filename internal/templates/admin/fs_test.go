package admintmpl_test

import (
	"io/fs"
	"testing"

	admintmpl "busca-cnpj-2026/internal/templates/admin"
)

func TestEmbeddedTemplates(t *testing.T) {
	entries, err := fs.ReadDir(admintmpl.Files, ".")
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) < 7 {
		t.Fatalf("expected at least 7 templates, got %d", len(entries))
	}
}
