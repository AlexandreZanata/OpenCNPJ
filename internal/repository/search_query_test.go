package repository

import "testing"

func TestFuzzyRazaoSocialSQL(t *testing.T) {
	where := fuzzyRazaoSocialWhere(2)
	if where != " AND razao_social % $2" {
		t.Fatalf("where = %q", where)
	}
	order := fuzzyRazaoSocialOrder(2)
	if order != "similarity(razao_social, $2) DESC, cnpj_basico ASC" {
		t.Fatalf("order = %q", order)
	}
}

func TestDetectTextSearchMode(t *testing.T) {
	if detectTextSearchMode("PETROBRAS") != textSearchTrigram {
		t.Fatal("single word should use trigram")
	}
	if detectTextSearchMode("PETRO BRAS") != textSearchFTS {
		t.Fatal("multi word should use fts")
	}
}

func TestFTSRazaoSocialSQL(t *testing.T) {
	where := ftsRazaoSocialWhere(2)
	if where != " AND busca @@ plainto_tsquery('portuguese', $2)" {
		t.Fatalf("where = %q", where)
	}
	order := ftsRazaoSocialOrder(2)
	if order != "ts_rank(busca, plainto_tsquery('portuguese', $2)) DESC, cnpj_basico ASC" {
		t.Fatalf("order = %q", order)
	}
}

func TestFuzzyNomeFantasiaSQL(t *testing.T) {
	where := fuzzyNomeFantasiaWhere(3)
	if where != " AND e.nome_fantasia % $3" {
		t.Fatalf("where = %q", where)
	}
	order := fuzzyNomeFantasiaOrder(3)
	if order != "similarity(e.nome_fantasia, $3) DESC, e.id ASC" {
		t.Fatalf("order = %q", order)
	}
}
