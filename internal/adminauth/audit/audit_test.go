package audit_test

import (
	"context"
	"testing"

	"github.com/google/uuid"

	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/adminauth/audit"
)

type stubQuerier struct {
	called bool
	params saasdb.InsertAdminAuditLogParams
}

func (s *stubQuerier) InsertAdminAuditLog(_ context.Context, p saasdb.InsertAdminAuditLogParams) (saasdb.AdminAuditLog, error) {
	s.called = true
	s.params = p
	return saasdb.AdminAuditLog{ID: 1}, nil
}

func TestLoggerWritesAuditRow(t *testing.T) {
	q := &stubQuerier{}
	log := audit.NewLogger(q)
	adminID := uuid.New()
	if err := log.Log(context.Background(), adminID, audit.ActionKeyCreated, "api_key", "kid", nil); err != nil {
		t.Fatal(err)
	}
	if !q.called || q.params.Action != audit.ActionKeyCreated {
		t.Fatalf("audit not written: %+v", q.params)
	}
	if !q.params.AdminID.Valid {
		t.Fatal("expected admin id")
	}
}
