package saas_test

import (
	"strings"
	"testing"

	"busca-cnpj-2026/internal/saas"
)

func TestMaskAPIKeyLong(t *testing.T) {
	raw := saas.KeyPrefix + strings.Repeat("a", saas.KeyHexLength)
	got := saas.MaskAPIKey(raw)
	if !strings.HasSuffix(got, "...") || len(got) != 19 {
		t.Fatalf("mask = %q", got)
	}
}

func TestMaskAPIKeyEmpty(t *testing.T) {
	if saas.MaskAPIKey("") != "" {
		t.Fatal("expected empty")
	}
}
