package domain

import "github.com/google/uuid"

// AdminUser is the authenticated admin identity.
type AdminUser struct {
	ID         uuid.UUID
	Email      string
	MFAEnabled bool
	Role       string
}

// SessionClaims are embedded in the access JWT.
type SessionClaims struct {
	AdminID     uuid.UUID
	Role        string
	MFAVerified bool
}
