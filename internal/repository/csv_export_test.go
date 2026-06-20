package repository

import (
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestTextSelectExpr(t *testing.T) {
	got := textSelectExpr("COALESCE(e.uf, '')", "uf")
	want := "(COALESCE(e.uf, ''))::TEXT AS uf"
	if got != want {
		t.Fatalf("textSelectExpr = %q, want %q", got, want)
	}
}

func TestHasFuzzyTextFilter(t *testing.T) {
	if !hasFuzzyTextFilter(models.SearchFilters{RazaoSocial: "LTDA"}) {
		t.Fatal("expected razao_social to trigger fuzzy filter")
	}
	if !hasFuzzyTextFilter(models.SearchFilters{NomeFantasia: "MERCADO"}) {
		t.Fatal("expected nome_fantasia to trigger fuzzy filter")
	}
	if hasFuzzyTextFilter(models.SearchFilters{UF: "SP", CNPJBasico: "12345678"}) {
		t.Fatal("expected exact filters not to trigger fuzzy filter")
	}
}

func TestFuzzySearchTotal(t *testing.T) {
	if total := fuzzySearchTotal(0, 10, 11); total != 11 {
		t.Fatalf("hasMore total = %d, want 11", total)
	}
	if total := fuzzySearchTotal(20, 10, 5); total != 25 {
		t.Fatalf("exact total = %d, want 25", total)
	}
}
