package services

import (
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestMeiliEligibleEmpresa(t *testing.T) {
	f := models.SearchFilters{RazaoSocial: "PETROBRAS", Limit: 20}
	if !meiliEligibleEmpresa(f) {
		t.Fatal("expected eligible")
	}
	f.Cursor = "x"
	if meiliEligibleEmpresa(f) {
		t.Fatal("cursor should disable meili")
	}
}

func TestMeiliEligibleEstabelecimento(t *testing.T) {
	f := models.SearchFilters{NomeFantasia: "PADARIA", Limit: 20}
	if !meiliEligibleEstabelecimento(f) {
		t.Fatal("expected eligible")
	}
	f.UF = "SP"
	if meiliEligibleEstabelecimento(f) {
		t.Fatal("uf filter should disable meili")
	}
}
