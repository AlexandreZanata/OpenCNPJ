package adminstatic_test

import (
	"io/fs"
	"testing"

	adminstatic "busca-cnpj-2026/internal/static/admin"
)

func TestEmbeddedCSSUnder8KB(t *testing.T) {
	data, err := fs.ReadFile(adminstatic.Files, "admin.css")
	if err != nil {
		t.Fatal(err)
	}
	if len(data) > 8*1024 {
		t.Fatalf("admin.css too large: %d bytes", len(data))
	}
	if len(data) == 0 {
		t.Fatal("empty css")
	}
}
