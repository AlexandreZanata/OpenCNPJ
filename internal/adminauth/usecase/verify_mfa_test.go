package usecase

import (
	"testing"

	"busca-cnpj-2026/internal/adminauth"
	totpsvc "busca-cnpj-2026/internal/adminauth/totp"
)

func TestMFACodeValidBypass(t *testing.T) {
	totp := totpsvc.NewService("OpenCNPJ-Admin")
	cfg := adminauth.Config{MFABypassCode: "000000"}

	if !mfaCodeValid(cfg, totp, "unused-secret", "000000") {
		t.Fatal("expected bypass code 000000 to be accepted")
	}
	if mfaCodeValid(cfg, totp, "unused-secret", "000001") {
		t.Fatal("expected non-bypass code to be rejected")
	}
}

func TestMFACodeValidBypassDisabled(t *testing.T) {
	totp := totpsvc.NewService("OpenCNPJ-Admin")
	cfg := adminauth.Config{}

	if mfaCodeValid(cfg, totp, "unused-secret", "000000") {
		t.Fatal("expected 000000 rejected when bypass is unset")
	}
}
