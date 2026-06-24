package repository

import (
	"testing"
)

func TestParseSearchCursor(t *testing.T) {
	parts, err := parseSearchCursor("score:0.45000000|cnpj:12345678")
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if parts["score"] != "0.45000000" || parts["cnpj"] != "12345678" {
		t.Fatalf("parts = %#v", parts)
	}
}

func TestBuildScoreCNPJCursor(t *testing.T) {
	got := buildScoreCNPJCursor(0.45, "12345678")
	want := "score:0.45000000|cnpj:12345678"
	if got != want {
		t.Fatalf("cursor = %q, want %q", got, want)
	}
}

func TestUseKeysetPagination(t *testing.T) {
	if !useKeysetPagination("cnpj:1", 0) {
		t.Fatal("expected keyset when cursor set and offset zero")
	}
	if useKeysetPagination("cnpj:1", 5) {
		t.Fatal("offset with cursor should not use keyset in handler layer")
	}
}

func TestEmpresaKeysetClauseCNPJ(t *testing.T) {
	args := []interface{}{}
	argPos := 3
	clause, err := empresaKeysetClause("cnpj:99999999", 0, false, &argPos, &args)
	if err != nil {
		t.Fatalf("clause: %v", err)
	}
	if clause != " AND cnpj_basico > $3" {
		t.Fatalf("clause = %q", clause)
	}
	if len(args) != 1 || args[0] != "99999999" {
		t.Fatalf("args = %#v", args)
	}
}
