package usecase

import (
	"context"
	"errors"
	"fmt"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/bruteforce"
	"busca-cnpj-2026/internal/adminauth/challenge"
	"busca-cnpj-2026/internal/adminauth/password"
)

// LoginInput is the credential payload for step 1.
type LoginInput struct {
	Email    string
	Password string
}

// LoginMFARequired is returned when password is valid and MFA is pending.
type LoginMFARequired struct {
	ChallengeID      uuid.UUID
	ExpiresInSeconds int
}

// LoginDeps are collaborators for the login use case.
type LoginDeps struct {
	Repo    adminRepo
	Guard   *bruteforce.Guard
	ChStore *challenge.Store
	Cfg     adminauth.Config
}

type adminRepo interface {
	GetByEmail(ctx context.Context, email string) (adminauth.AdminRow, error)
}

// Login verifies credentials and starts an MFA challenge when enabled.
func Login(ctx context.Context, d LoginDeps, in LoginInput) (LoginMFARequired, error) {
	email := normalizeEmail(in.Email)
	if locked, err := d.Guard.IsLocked(ctx, email); err != nil {
		return LoginMFARequired{}, err
	} else if locked {
		return LoginMFARequired{}, autherr.ErrAccountLocked
	}
	row, err := d.Repo.GetByEmail(ctx, email)
	if err != nil {
		if errors.Is(err, pgx.ErrNoRows) {
			_ = d.Guard.RecordFailure(ctx, email)
			return LoginMFARequired{}, autherr.ErrInvalidCredentials
		}
		return LoginMFARequired{}, fmt.Errorf("load admin: %w", err)
	}
	ok, err := password.VerifyBytes(row.PasswordHash, in.Password)
	if err != nil || !ok {
		_ = d.Guard.RecordFailure(ctx, email)
		return LoginMFARequired{}, autherr.ErrInvalidCredentials
	}
	_ = d.Guard.ClearFailures(ctx, email)
	if !row.MFAEnabled {
		return LoginMFARequired{}, autherr.ErrMFARequired
	}
	chID, err := d.ChStore.Create(ctx, row.ID, row.Email)
	if err != nil {
		return LoginMFARequired{}, err
	}
	return LoginMFARequired{
		ChallengeID:      chID,
		ExpiresInSeconds: d.Cfg.ChallengeTTLSeconds,
	}, nil
}
