package domain_test

import (
	"testing"

	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/domain"
)

func TestSessionClaimsFields(t *testing.T) {
	id := uuid.New()
	c := domain.SessionClaims{AdminID: id, Role: "super_admin", MFAVerified: true}
	if c.AdminID != id || !c.MFAVerified {
		t.Fatal("claims mismatch")
	}
}
