package database_test

import (
	"strings"
	"testing"

	"busca-cnpj-2026/internal/database"
)

func TestCNPJSessionSetupDisablesJITAndSetsTimeout(t *testing.T) {
	src := database.CNPJSessionSetup
	if !strings.Contains(src, "jit = off") {
		t.Fatalf("expected jit disabled, got %q", src)
	}
	if !strings.Contains(src, "statement_timeout") {
		t.Fatalf("expected statement_timeout, got %q", src)
	}
}
