package loader

import (
	"context"
	"math"
	"runtime"
	"time"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type PGCopier struct {
	pool *pgxpool.Pool
}

func NewPoolConfig(databaseURL string) (*pgxpool.Config, error) {
	cfg, err := pgxpool.ParseConfig(databaseURL)
	if err != nil {
		return nil, err
	}

	maxConns := runtime.NumCPU() * 2
	minConns := runtime.NumCPU()
	if maxConns > math.MaxInt32 {
		maxConns = math.MaxInt32
	}
	if minConns > math.MaxInt32 {
		minConns = math.MaxInt32
	}

	cfg.MaxConns = int32(maxConns) //nolint:gosec // Value is clamped to MaxInt32 above.
	cfg.MinConns = int32(minConns) //nolint:gosec // Value is clamped to MaxInt32 above.
	cfg.MaxConnLifetime = 30 * time.Minute
	return cfg, nil
}

func NewPGCopier(ctx context.Context, databaseURL string) (*PGCopier, error) {
	cfg, err := NewPoolConfig(databaseURL)
	if err != nil {
		return nil, err
	}
	pool, err := pgxpool.NewWithConfig(ctx, cfg)
	if err != nil {
		return nil, err
	}
	return &PGCopier{pool: pool}, nil
}

func (c *PGCopier) Close() {
	if c.pool != nil {
		c.pool.Close()
	}
}

func (c *PGCopier) CopyRows(
	ctx context.Context,
	schema string,
	table string,
	columns []string,
	rows [][]any,
) (int64, error) {
	return c.pool.CopyFrom(
		ctx,
		pgx.Identifier{schema, table},
		columns,
		pgx.CopyFromRows(rows),
	)
}
