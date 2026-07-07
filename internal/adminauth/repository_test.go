package adminauth_test

import (
	"testing"

	"busca-cnpj-2026/internal/adminauth"
)

func TestNewRefreshTokenUnique(t *testing.T) {
	a, err := adminauth.NewRefreshToken()
	if err != nil {
		t.Fatal(err)
	}
	b, err := adminauth.NewRefreshToken()
	if err != nil {
		t.Fatal(err)
	}
	if a == b || len(a) < 32 {
		t.Fatalf("unexpected tokens: %q %q", a, b)
	}
}
