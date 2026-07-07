package saas

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgtype"

	saasdb "busca-cnpj-2026/internal/db/saas"
)

// Authenticator validates raw API keys against the SaaS database.
type Authenticator interface {
	Authenticate(ctx context.Context, rawKey string) (AuthenticatedClient, error)
}

// KeyStore persists and resolves API keys via sqlc + pgx.
type KeyStore struct {
	queries saasdb.Querier
}

// NewKeyStore returns an Authenticator backed by sqlc queries.
func NewKeyStore(queries saasdb.Querier) *KeyStore {
	return &KeyStore{queries: queries}
}

// Authenticate resolves a plaintext key to client metadata.
func (s *KeyStore) Authenticate(ctx context.Context, rawKey string) (AuthenticatedClient, error) {
	if err := ValidateKeyFormat(rawKey); err != nil {
		return AuthenticatedClient{}, err
	}
	hash := HashKey(rawKey)
	row, err := s.queries.GetAPIKeyByHash(ctx, hash)
	if err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			return AuthenticatedClient{}, ErrInvalidKey
		}
		return AuthenticatedClient{}, fmt.Errorf("lookup api key: %w", err)
	}
	if row.ExpiresAt.Valid && row.ExpiresAt.Time.Before(time.Now()) {
		return AuthenticatedClient{}, ErrExpiredKey
	}
	if row.Status != ClientStatusActive {
		return AuthenticatedClient{}, ErrSuspendedClient
	}
	keyID, err := uuidFromPg(row.ID)
	if err != nil {
		return AuthenticatedClient{}, err
	}
	clientID, err := uuidFromPg(row.ClientID)
	if err != nil {
		return AuthenticatedClient{}, err
	}
	return AuthenticatedClient{
		KeyID:           keyID,
		ClientID:        clientID,
		KeyPrefix:       row.KeyPrefix,
		RateLimitPerMin: int(row.RateLimitPerMin),
		MonthlyQuota:    int(row.MonthlyQuota),
		Status:          row.Status,
	}, nil
}

// GenerateKey builds a new live API key (plaintext returned once).
func GenerateKey() (string, error) {
	buf := make([]byte, KeyHexLength/2)
	if _, err := rand.Read(buf); err != nil {
		return "", fmt.Errorf("rand: %w", err)
	}
	return KeyPrefix + hex.EncodeToString(buf), nil
}

// HashKey returns SHA-256 of the full API key.
func HashKey(raw string) []byte {
	sum := sha256.Sum256([]byte(raw))
	return sum[:]
}

// KeyDisplayPrefix returns the first 16 chars for storage/display.
func KeyDisplayPrefix(raw string) string {
	if len(raw) <= 16 {
		return raw
	}
	return raw[:16]
}

// ValidateKeyFormat checks prefix and hex suffix length.
func ValidateKeyFormat(raw string) error {
	if !strings.HasPrefix(raw, KeyPrefix) {
		return ErrInvalidKey
	}
	suffix := strings.TrimPrefix(raw, KeyPrefix)
	if len(suffix) != KeyHexLength {
		return ErrInvalidKey
	}
	if _, err := hex.DecodeString(suffix); err != nil {
		return ErrInvalidKey
	}
	return nil
}

// CreateClientKey inserts a client and key (tests, admin CLI seed).
func CreateClientKey(
	ctx context.Context,
	q saasdb.Querier,
	name, email, label string,
	ratePerMin, monthlyQuota int32,
) (plainKey string, clientID uuid.UUID, err error) {
	client, err := q.InsertAPIClient(ctx, saasdb.InsertAPIClientParams{
		Name:            name,
		Email:           email,
		Status:          ClientStatusActive,
		MonthlyQuota:    monthlyQuota,
		RateLimitPerMin: ratePerMin,
	})
	if err != nil {
		return "", uuid.Nil, fmt.Errorf("insert client: %w", err)
	}
	plainKey, err = GenerateKey()
	if err != nil {
		return "", uuid.Nil, err
	}
	pgClientID := client.ID
	_, err = q.InsertAPIKey(ctx, saasdb.InsertAPIKeyParams{
		ClientID:  pgClientID,
		KeyPrefix: KeyDisplayPrefix(plainKey),
		KeyHash:   HashKey(plainKey),
		Label:     label,
	})
	if err != nil {
		return "", uuid.Nil, fmt.Errorf("insert key: %w", err)
	}
	id, err := uuidFromPg(client.ID)
	if err != nil {
		return "", uuid.Nil, err
	}
	return plainKey, id, nil
}

func uuidFromPg(u pgtype.UUID) (uuid.UUID, error) {
	if !u.Valid {
		return uuid.Nil, fmt.Errorf("invalid uuid")
	}
	return uuid.FromBytes(u.Bytes[:])
}
