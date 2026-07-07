package saas_test

import (
	"testing"

	"busca-cnpj-2026/internal/saas"
)

func TestSecureCompareKeyHashMatch(t *testing.T) {
	a := saas.HashKey("ocnpj_live_" + repeatHex(32))
	b := saas.HashKey("ocnpj_live_" + repeatHex(32))
	if !saas.SecureCompareKeyHash(a, b) {
		t.Fatal("expected match for identical keys")
	}
}

func TestSecureCompareKeyHashMismatch(t *testing.T) {
	a := saas.HashKey("ocnpj_live_" + repeatHex(32))
	b := saas.HashKey("ocnpj_live_" + repeatHex(31) + "0")
	if saas.SecureCompareKeyHash(a, b) {
		t.Fatal("expected mismatch")
	}
}

func TestSecureCompareKeyHashWrongLength(t *testing.T) {
	if saas.SecureCompareKeyHash([]byte{1}, saas.HashKey("ocnpj_live_"+repeatHex(32))) {
		t.Fatal("expected false for wrong stored length")
	}
}

func repeatHex(n int) string {
	const hex = "0123456789abcdef"
	out := make([]byte, n)
	for i := range out {
		out[i] = hex[i%16]
	}
	return string(out)
}
