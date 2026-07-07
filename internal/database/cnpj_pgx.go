package database

import (
	"context"
	"fmt"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"

	"busca-cnpj-2026/internal/config"
)

// CNPJPool is the pgx v5 read pool for opencnpj_cnpj (public API hot path).
var CNPJPool *pgxpool.Pool

// InitCNPJPgx opens the CNPJ consulta pgx pool when SaaS mode is enabled.
func InitCNPJPgx() error {
	if config.AppConfig == nil || !config.AppConfig.SaaS.Enabled {
		return nil
	}
	url := config.GetCNPJDatabaseURL()
	if url == "" {
		return fmt.Errorf("saas.enabled requires database_cnpj.url or CNPJ_DATABASE_URL")
	}
	cfg, err := pgxpool.ParseConfig(url)
	if err != nil {
		return fmt.Errorf("parse cnpj dsn: %w", err)
	}
	pool := config.AppConfig.DatabaseCNPJ.AsPoolConfig()
	if pool.MaxOpenConns > 0 {
		cfg.MaxConns = int32(pool.MaxOpenConns)
	}
	if pool.MaxIdleConns > 0 {
		cfg.MinConns = int32(pool.MaxIdleConns)
	}
	CNPJPool, err = pgxpool.NewWithConfig(context.Background(), cfg)
	if err != nil {
		return fmt.Errorf("open cnpj pool: %w", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := CNPJPool.Ping(ctx); err != nil {
		CNPJPool.Close()
		CNPJPool = nil
		return fmt.Errorf("ping cnpj pool: %w", err)
	}
	return nil
}

// CloseCNPJPgx closes the CNPJ pgx pool.
func CloseCNPJPgx() {
	if CNPJPool == nil {
		return
	}
	CNPJPool.Close()
	CNPJPool = nil
}
