package services

import (
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestExportUsesEstabelecimentosByUFFilter(t *testing.T) {
	filters := models.SearchFilters{UF: "SP"}
	columns := []string{"cnpj_completo", "uf"}
	if !exportUsesEstabelecimentos(filters, columns) {
		t.Fatal("expected UF filter to route export to estabelecimentos")
	}
}

func TestExportUsesEmpresasForBasicoOnly(t *testing.T) {
	filters := models.SearchFilters{CNPJBasico: "12345678"}
	columns := []string{"cnpj_basico", "razao_social"}
	if exportUsesEstabelecimentos(filters, columns) {
		t.Fatal("expected cnpj_basico export to route to empresas")
	}
}

func TestExportUsesEstabelecimentosByColumn(t *testing.T) {
	filters := models.SearchFilters{}
	columns := []string{"cnpj_completo"}
	if !exportUsesEstabelecimentos(filters, columns) {
		t.Fatal("expected cnpj_completo column to route export to estabelecimentos")
	}
}
