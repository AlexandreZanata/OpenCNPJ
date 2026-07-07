package app

import (
	"context"
	"fmt"
	"time"

	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/bruteforce"
	"busca-cnpj-2026/internal/adminauth/challenge"
	"busca-cnpj-2026/internal/adminauth/cipher"
	adminhandlers "busca-cnpj-2026/internal/adminauth/handlers"
	"busca-cnpj-2026/internal/adminauth/token"
	totpsvc "busca-cnpj-2026/internal/adminauth/totp"
	"busca-cnpj-2026/internal/adminauth/usecase"
	"busca-cnpj-2026/internal/config"
	saasdb "busca-cnpj-2026/internal/db/saas"
)

// Deps holds wired admin auth collaborators.
type Deps struct {
	Handler *adminhandlers.AuthHandler
	Signer  *token.RS256Signer
	Config  adminauth.Config
}

// Wire builds admin auth dependencies when SaaS admin is enabled.
func Wire(_ context.Context, queries saasdb.Querier, rdb *redis.Client, saasCfg config.SaasConfig) (*Deps, error) {
	if !saasCfg.AdminEnabled {
		return nil, autherr.ErrAdminDisabled
	}
	if rdb == nil {
		return nil, fmt.Errorf("admin auth requires redis")
	}
	cfg, err := adminauth.LoadConfig(saasCfg.AdminJWTTTLMinutes, saasCfg.AdminRefreshTTLDays, saasCfg.MFATOTPIssuer)
	if err != nil {
		return nil, err
	}
	signer, err := token.NewRS256Signer(
		cfg.JWTPrivateKeyPath,
		cfg.JWTPublicKeyPath,
		time.Duration(cfg.AccessTTLMinutes)*time.Minute,
		cfg.Role,
	)
	if err != nil {
		return nil, err
	}
	aead, err := cipher.NewAESGCM(cfg.MFASecretKey)
	if err != nil {
		return nil, err
	}
	repo := adminauth.NewAdminRepository(queries)
	guard := bruteforce.NewGuard(rdb, cfg.MaxLoginFailures, cfg.LockoutMinutes)
	chStore := challenge.NewStore(rdb, cfg.ChallengeTTLSeconds)
	totpSvc := totpsvc.NewService(cfg.TOTPIssuer)

	loginDeps := usecase.LoginDeps{Repo: repo, Guard: guard, ChStore: chStore, Cfg: cfg}
	verifyDeps := usecase.VerifyMFADeps{
		Repo: repo, ChStore: chStore, Cipher: aead, TOTP: totpSvc, Signer: signer, Cfg: cfg,
	}
	refreshDeps := usecase.RefreshDeps{Repo: repo, Signer: signer, Cfg: cfg}

	handler := adminhandlers.NewAuthHandler(
		func(ctx context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error) {
			return usecase.Login(ctx, loginDeps, in)
		},
		func(ctx context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error) {
			return usecase.VerifyMFA(ctx, verifyDeps, in)
		},
		func(ctx context.Context, raw string) (usecase.AuthTokens, error) {
			return usecase.Refresh(ctx, refreshDeps, raw)
		},
		cfg.RefreshCookieName,
	)
	return &Deps{Handler: handler, Signer: signer, Config: cfg}, nil
}
