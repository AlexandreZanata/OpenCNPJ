package admin

import (
	"context"

	adminapp "busca-cnpj-2026/internal/adminauth/app"
	"busca-cnpj-2026/internal/adminauth/audit"
	"busca-cnpj-2026/internal/adminauth/usecase"
	"busca-cnpj-2026/internal/apidocs"
	"busca-cnpj-2026/internal/config"
	saasdb "busca-cnpj-2026/internal/db/saas"
)

// WirePanel builds admin panel handler from auth deps and SaaS queries.
func WirePanel(auth *adminapp.Deps, queries saasdb.Querier, saasCfg config.SaasConfig) (*Handler, error) {
	r, err := NewRenderer()
	if err != nil {
		return nil, err
	}
	store := NewSession()
	docsURL := saasCfg.DocsPublicURL
	if docsURL == "" {
		docsURL = apidocs.DefaultPublicDocsURL
	}
	return NewHandler(Deps{
		Queries: queries,
		Login: func(ctx context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error) {
			return auth.Handler.Login(ctx, in)
		},
		Verify: func(ctx context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error) {
			return auth.Handler.Verify(ctx, in)
		},
		Signer:        auth.Signer,
		Session:       store,
		RefreshCookie: auth.Config.RefreshCookieName,
		DefaultRate:   int32(saasCfg.DefaultClientRateMin),
		DefaultQuota:  int32(saasCfg.DefaultMonthlyQuota),
		Renderer:      r,
		DocsPublicURL: docsURL,
		Audit:         audit.NewLogger(queries),
	}), nil
}
