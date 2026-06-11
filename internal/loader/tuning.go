package loader

import (
	"context"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

// WithImportTuning configures pool connections for bulk COPY workloads.
func WithImportTuning(cfg *pgxpool.Config) {
	cfg.AfterConnect = func(ctx context.Context, conn *pgx.Conn) error {
		_, err := conn.Exec(ctx, `
			SET synchronous_commit = off;
			SET session_replication_role = replica;
			SET work_mem = '256MB';
			SET maintenance_work_mem = '2GB';
		`)
		return err
	}
}

func NewImportPoolConfig(databaseURL string, tuned bool) (*pgxpool.Config, error) {
	cfg, err := NewPoolConfig(databaseURL)
	if err != nil {
		return nil, err
	}
	if tuned {
		WithImportTuning(cfg)
	}
	return cfg, nil
}
