package repository

import (
	"database/sql"
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestBuildEmpresaAggregates(t *testing.T) {
	empresas := []models.Empresa{{CNPJBasico: "12345678", RazaoSocial: "ACME"}}
	full := map[string]models.EmpresaFull{
		"12345678": {Empresa: empresas[0], NaturezaDescricao: sqlNullString("LTDA")},
	}
	agg := BuildEmpresaAggregates(empresas, full, nil, nil, nil)
	if len(agg) != 1 {
		t.Fatalf("len = %d", len(agg))
	}
	if len(agg[0].Estabelecimentos) != 0 || len(agg[0].Socios) != 0 {
		t.Fatal("expected empty related slices")
	}
}

func sqlNullString(v string) sql.NullString {
	return sql.NullString{String: v, Valid: true}
}
