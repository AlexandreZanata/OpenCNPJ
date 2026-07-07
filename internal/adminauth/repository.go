package adminauth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgtype"

	saasdb "busca-cnpj-2026/internal/db/saas"
)

// AdminRepository wraps sqlc admin queries.
type AdminRepository struct {
	q saasdb.Querier
}

// NewAdminRepository returns a repository backed by sqlc.
func NewAdminRepository(q saasdb.Querier) *AdminRepository {
	return &AdminRepository{q: q}
}

// AdminRow is a loaded admin user with password hash.
type AdminRow struct {
	ID           uuid.UUID
	Email        string
	PasswordHash []byte
	MFAEnabled   bool
}

// GetByEmail loads an admin by email.
func (r *AdminRepository) GetByEmail(ctx context.Context, email string) (AdminRow, error) {
	row, err := r.q.GetAdminUserByEmail(ctx, email)
	if err != nil {
		return AdminRow{}, err
	}
	id, err := uuidFromPg(row.ID)
	if err != nil {
		return AdminRow{}, err
	}
	return AdminRow{
		ID:           id,
		Email:        row.Email,
		PasswordHash: row.PasswordHash,
		MFAEnabled:   row.MfaEnabled,
	}, nil
}

// UpsertAdmin creates or updates an admin user.
func (r *AdminRepository) UpsertAdmin(ctx context.Context, email string, hash []byte, mfa bool) (AdminRow, error) {
	row, err := r.q.UpsertAdminUser(ctx, saasdb.UpsertAdminUserParams{
		Email:        email,
		PasswordHash: hash,
		MfaEnabled:   mfa,
	})
	if err != nil {
		return AdminRow{}, err
	}
	id, err := uuidFromPg(row.ID)
	if err != nil {
		return AdminRow{}, err
	}
	return AdminRow{ID: id, Email: row.Email, PasswordHash: row.PasswordHash, MFAEnabled: row.MfaEnabled}, nil
}

// SaveMFASecret stores encrypted TOTP secret bytes.
func (r *AdminRepository) SaveMFASecret(ctx context.Context, adminID uuid.UUID, enc []byte) error {
	return r.q.UpsertAdminMFASecret(ctx, saasdb.UpsertAdminMFASecretParams{
		AdminID:         pgUUID(adminID),
		SecretEncrypted: enc,
	})
}

// LoadMFASecret returns encrypted TOTP secret bytes.
func (r *AdminRepository) LoadMFASecret(ctx context.Context, adminID uuid.UUID) ([]byte, error) {
	row, err := r.q.GetAdminMFASecret(ctx, pgUUID(adminID))
	if err != nil {
		return nil, err
	}
	return row.SecretEncrypted, nil
}

// StoreRefreshToken persists a hashed refresh token.
func (r *AdminRepository) StoreRefreshToken(ctx context.Context, adminID uuid.UUID, token string, expires time.Time) error {
	_, err := r.q.InsertAdminRefreshToken(ctx, saasdb.InsertAdminRefreshTokenParams{
		AdminID:   pgUUID(adminID),
		TokenHash: hashToken(token),
		ExpiresAt: pgTimestamptz(expires),
	})
	return err
}

// FindRefreshToken loads a valid refresh token row ID for the raw token.
func (r *AdminRepository) FindRefreshToken(ctx context.Context, token string) (uuid.UUID, uuid.UUID, error) {
	row, err := r.q.GetValidRefreshToken(ctx, hashToken(token))
	if err != nil {
		return uuid.Nil, uuid.Nil, err
	}
	tid, err := uuidFromPg(row.ID)
	if err != nil {
		return uuid.Nil, uuid.Nil, err
	}
	aid, err := uuidFromPg(row.AdminID)
	if err != nil {
		return uuid.Nil, uuid.Nil, err
	}
	return tid, aid, nil
}

// RevokeRefreshToken marks a refresh token as revoked.
func (r *AdminRepository) RevokeRefreshToken(ctx context.Context, tokenID uuid.UUID) error {
	return r.q.RevokeRefreshToken(ctx, pgUUID(tokenID))
}

// NewRefreshToken generates a random opaque refresh token.
func NewRefreshToken() (string, error) {
	buf := make([]byte, 32)
	if _, err := rand.Read(buf); err != nil {
		return "", fmt.Errorf("rand: %w", err)
	}
	return hex.EncodeToString(buf), nil
}

func hashToken(token string) []byte {
	sum := sha256.Sum256([]byte(token))
	return sum[:]
}

func pgUUID(id uuid.UUID) pgtype.UUID {
	return pgtype.UUID{Bytes: id, Valid: true}
}

func pgTimestamptz(t time.Time) pgtype.Timestamptz {
	return pgtype.Timestamptz{Time: t, Valid: true}
}

func uuidFromPg(u pgtype.UUID) (uuid.UUID, error) {
	if !u.Valid {
		return uuid.Nil, fmt.Errorf("invalid uuid")
	}
	return uuid.FromBytes(u.Bytes[:])
}
