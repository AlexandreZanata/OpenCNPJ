package usecase

import (
	"context"
	"time"

	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/token"
)

// RefreshDeps are collaborators for token refresh.
type RefreshDeps struct {
	Repo   refreshRepo
	Signer *token.RS256Signer
	Cfg    adminauth.Config
}

type refreshRepo interface {
	FindRefreshToken(ctx context.Context, token string) (uuid.UUID, uuid.UUID, error)
	RevokeRefreshToken(ctx context.Context, tokenID uuid.UUID) error
	StoreRefreshToken(ctx context.Context, adminID uuid.UUID, token string, expires time.Time) error
}

// Refresh rotates refresh token and issues a new access JWT.
func Refresh(ctx context.Context, d RefreshDeps, rawRefresh string) (AuthTokens, error) {
	tokenID, adminID, err := d.Repo.FindRefreshToken(ctx, rawRefresh)
	if err != nil {
		return AuthTokens{}, autherr.ErrInvalidToken
	}
	access, ttl, err := d.Signer.SignAccessToken(adminID, true)
	if err != nil {
		return AuthTokens{}, err
	}
	newRefresh, err := adminauth.NewRefreshToken()
	if err != nil {
		return AuthTokens{}, err
	}
	refreshExp := time.Now().Add(time.Duration(d.Cfg.RefreshTTLDays) * 24 * time.Hour)
	if err := d.Repo.RevokeRefreshToken(ctx, tokenID); err != nil {
		return AuthTokens{}, err
	}
	if err := d.Repo.StoreRefreshToken(ctx, adminID, newRefresh, refreshExp); err != nil {
		return AuthTokens{}, err
	}
	return AuthTokens{
		AccessToken:      access,
		ExpiresInSeconds: ttl,
		RefreshToken:     newRefresh,
		RefreshExpires:   refreshExp,
	}, nil
}
