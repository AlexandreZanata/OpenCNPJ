package repository

import "testing"

func TestFuzzyRazaoSocialSQL(t *testing.T) {
	where := fuzzyRazaoSocialWhere(2)
	if where != " AND razao_social % $2" {
		t.Fatalf("where = %q", where)
	}
	order := fuzzyRazaoSocialOrder(2)
	if order != "similarity(razao_social, $2) DESC" {
		t.Fatalf("order = %q", order)
	}
}

func TestFuzzyNomeFantasiaSQL(t *testing.T) {
	where := fuzzyNomeFantasiaWhere(3)
	if where != " AND e.nome_fantasia % $3" {
		t.Fatalf("where = %q", where)
	}
	order := fuzzyNomeFantasiaOrder(3)
	if order != "similarity(e.nome_fantasia, $3) DESC" {
		t.Fatalf("order = %q", order)
	}
}
