package cnpj_test

import (
	"testing"

	"busca-cnpj-2026/internal/cnpj"
)

func TestValidateKnownCNPJ(t *testing.T) {
	if err := cnpj.Validate("00000000000191"); err != nil {
		t.Fatalf("expected valid CNPJ: %v", err)
	}
}

func TestValidateRejectsShortInput(t *testing.T) {
	if err := cnpj.Validate("123"); err == nil {
		t.Fatal("expected invalid cnpj")
	}
}

func TestValidateRejectsBadCheckDigits(t *testing.T) {
	if err := cnpj.Validate("00000000000190"); err == nil {
		t.Fatal("expected invalid check digits")
	}
}

func TestNormalizeStripsFormatting(t *testing.T) {
	got := cnpj.Normalize("00.000.000/0001-91")
	want := "00000000000191"
	if got != want {
		t.Fatalf("normalize = %q, want %q", got, want)
	}
}

func TestBasicoFromCompleto(t *testing.T) {
	got := cnpj.BasicoFromCompleto("00000000000191")
	if got != "00000000" {
		t.Fatalf("basico = %q", got)
	}
}
