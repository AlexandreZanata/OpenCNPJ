package usecase

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/challenge"
	"busca-cnpj-2026/internal/adminauth/cipher"
	"busca-cnpj-2026/internal/adminauth/token"
	totpsvc "busca-cnpj-2026/internal/adminauth/totp"
)

// VerifyMFAInput is the second login step payload.
type VerifyMFAInput struct {
	ChallengeID uuid.UUID
	Code        string
}

// AuthTokens is returned after successful MFA verification.
type AuthTokens struct {
	AccessToken      string
	ExpiresInSeconds int
	RefreshToken     string
	RefreshExpires   time.Time
}

// VerifyMFADeps are collaborators for MFA verification.
type VerifyMFADeps struct {
	Repo    mfaRepo
	ChStore *challenge.Store
	Cipher  *cipher.AESGCM
	TOTP    *totpsvc.Service
	Signer  *token.RS256Signer
	Cfg     adminauth.Config
}

type mfaRepo interface {
	LoadMFASecret(ctx context.Context, adminID uuid.UUID) ([]byte, error)
	StoreRefreshToken(ctx context.Context, adminID uuid.UUID, token string, expires time.Time) error
}

// VerifyMFA validates TOTP and issues access + refresh tokens.
func VerifyMFA(ctx context.Context, d VerifyMFADeps, in VerifyMFAInput) (AuthTokens, error) {
	payload, err := d.ChStore.Consume(ctx, in.ChallengeID)
	if err != nil {
		return AuthTokens{}, autherr.ErrInvalidChallenge
	}
	enc, err := d.Repo.LoadMFASecret(ctx, payload.AdminID)
	if err != nil {
		return AuthTokens{}, fmt.Errorf("load mfa secret: %w", err)
	}
	secretBytes, err := d.Cipher.Decrypt(enc)
	if err != nil {
		return AuthTokens{}, fmt.Errorf("decrypt mfa secret: %w", err)
	}
	code := strings.TrimSpace(in.Code)
	if !d.TOTP.Validate(string(secretBytes), code) {
		return AuthTokens{}, autherr.ErrInvalidMFA
	}
	access, ttl, err := d.Signer.SignAccessToken(payload.AdminID, true)
	if err != nil {
		return AuthTokens{}, err
	}
	refresh, err := adminauth.NewRefreshToken()
	if err != nil {
		return AuthTokens{}, err
	}
	refreshExp := time.Now().Add(time.Duration(d.Cfg.RefreshTTLDays) * 24 * time.Hour)
	if err := d.Repo.StoreRefreshToken(ctx, payload.AdminID, refresh, refreshExp); err != nil {
		return AuthTokens{}, err
	}
	return AuthTokens{
		AccessToken:      access,
		ExpiresInSeconds: ttl,
		RefreshToken:     refresh,
		RefreshExpires:   refreshExp,
	}, nil
}
