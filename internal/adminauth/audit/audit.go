package audit

import (
	"context"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgtype"

	saasdb "busca-cnpj-2026/internal/db/saas"
)

// Action names persisted in admin_audit_log.
const (
	ActionClientCreated   = "client.created"
	ActionClientSuspended = "client.suspended"
	ActionKeyCreated      = "key.created"
	ActionKeyRevoked      = "key.revoked"
	ActionLoginSuccess    = "admin.login.success"
	ActionLoginFailure    = "admin.login.failure"
	ActionMFAVerified     = "admin.mfa.verified"
)

// Logger writes admin audit events.
type Logger struct {
	q auditInserter
}

type auditInserter interface {
	InsertAdminAuditLog(ctx context.Context, arg saasdb.InsertAdminAuditLogParams) (saasdb.AdminAuditLog, error)
}

// NewLogger returns an audit logger backed by sqlc queries.
func NewLogger(q auditInserter) *Logger {
	return &Logger{q: q}
}

// Log records an audit event. adminID may be uuid.Nil for failed logins.
func (l *Logger) Log(
	ctx context.Context,
	adminID uuid.UUID,
	action, resourceType, resourceID string,
	details []byte,
) error {
	if l == nil || l.q == nil {
		return nil
	}
	_, err := l.q.InsertAdminAuditLog(ctx, saasdb.InsertAdminAuditLogParams{
		AdminID:      pgAdminID(adminID),
		Action:       action,
		ResourceType: pgText(resourceType),
		ResourceID:   pgText(resourceID),
		Details:      details,
	})
	return err
}

func pgAdminID(id uuid.UUID) pgtype.UUID {
	if id == uuid.Nil {
		return pgtype.UUID{}
	}
	return pgtype.UUID{Bytes: id, Valid: true}
}

func pgText(s string) pgtype.Text {
	if s == "" {
		return pgtype.Text{}
	}
	return pgtype.Text{String: s, Valid: true}
}
