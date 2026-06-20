package repository

import (
	"strings"
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestNormalizePhoneLimit(t *testing.T) {
	if got := normalizePhoneLimit(0); got != 5000 {
		t.Fatalf("default = %d, want 5000", got)
	}
	if got := normalizePhoneLimit(100000); got != 50000 {
		t.Fatalf("max cap = %d, want 50000", got)
	}
}

func TestFormatPhone(t *testing.T) {
	if got := formatPhone("11", "98765-4321"); got != "11987654321" {
		t.Fatalf("phone = %q", got)
	}
	if got := formatPhone("", "123"); got != "" {
		t.Fatalf("short phone should be empty, got %q", got)
	}
}

func TestBuildPhoneExportQueryRequiresFilter(t *testing.T) {
	_, _, err := buildPhoneExportQuery(models.PhoneExportRequest{Limit: 100})
	if err == nil {
		t.Fatal("expected error without category/cnae/nome_fantasia")
	}
}

func TestBuildPhoneExportQueryWithCategory(t *testing.T) {
	active := true
	query, args, err := buildPhoneExportQuery(models.PhoneExportRequest{
		Category:   "advocacia",
		UF:         "SP",
		OnlyActive: &active,
		Limit:      100,
	})
	if err != nil {
		t.Fatalf("build query: %v", err)
	}
	if !strings.Contains(query, "cnae_fiscal_principal") {
		t.Fatalf("expected CNAE filter in query: %s", query)
	}
	if len(args) == 0 {
		t.Fatal("expected args")
	}
}
