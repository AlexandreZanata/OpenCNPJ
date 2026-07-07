package saas_test

import (
	"strings"
	"testing"

	"busca-cnpj-2026/internal/saas"
)

func TestHashKeyDeterministic(t *testing.T) {
	raw := saas.KeyPrefix + strings.Repeat("a", saas.KeyHexLength)
	h1 := saas.HashKey(raw)
	h2 := saas.HashKey(raw)
	if len(h1) != 32 {
		t.Fatalf("hash len = %d, want 32", len(h1))
	}
	for i := range h1 {
		if h1[i] != h2[i] {
			t.Fatal("hash not deterministic")
		}
	}
}

func TestGenerateKeyFormat(t *testing.T) {
	key, err := saas.GenerateKey()
	if err != nil {
		t.Fatal(err)
	}
	if err := saas.ValidateKeyFormat(key); err != nil {
		t.Fatalf("generated key invalid: %v", err)
	}
	if !strings.HasPrefix(key, saas.KeyPrefix) {
		t.Fatalf("prefix missing: %s", key)
	}
}

func TestKeyDisplayPrefix(t *testing.T) {
	raw := saas.KeyPrefix + strings.Repeat("b", saas.KeyHexLength)
	got := saas.KeyDisplayPrefix(raw)
	if len(got) != 16 {
		t.Fatalf("display prefix len = %d", len(got))
	}
}

func TestValidateKeyFormatRejectsBadInput(t *testing.T) {
	cases := []string{"", "bad", "ocnpj_live_short", "ocnpj_test_" + strings.Repeat("c", saas.KeyHexLength)}
	for _, c := range cases {
		if err := saas.ValidateKeyFormat(c); err == nil {
			t.Fatalf("expected error for %q", c)
		}
	}
}
