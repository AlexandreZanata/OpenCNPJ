package handlers

import "testing"

func TestValidateFuzzySearchTerm(t *testing.T) {
	if err := validateFuzzySearchTerm("razao_social", ""); err != nil {
		t.Fatalf("empty term should pass: %v", err)
	}
	if err := validateFuzzySearchTerm("razao_social", "ab"); err == nil {
		t.Fatal("expected error for 2-char term")
	}
	if err := validateFuzzySearchTerm("nome_fantasia", "abc"); err != nil {
		t.Fatalf("3-char term should pass: %v", err)
	}
	if err := validateFuzzySearchTerm("nome_fantasia", "ação"); err != nil {
		t.Fatalf("unicode term with 4 runes should pass: %v", err)
	}
}
