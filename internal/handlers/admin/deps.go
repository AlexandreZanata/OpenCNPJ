package admin

import (
	"context"

	"github.com/gofiber/fiber/v2/middleware/session"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/token"
	"busca-cnpj-2026/internal/adminauth/usecase"
	saasdb "busca-cnpj-2026/internal/db/saas"
)

// Deps are admin panel dependencies.
type Deps struct {
	Queries       saasdb.Querier
	Login         func(ctx context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error)
	Verify        func(ctx context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error)
	Signer        *token.RS256Signer
	Session       *session.Store
	RefreshCookie string
	DefaultRate   int32
	DefaultQuota  int32
	Renderer      *Renderer
	DocsPublicURL string
	Audit         auditWriter
}

type auditWriter interface {
	Log(ctx context.Context, adminID uuid.UUID, action, resourceType, resourceID string, details []byte) error
}
