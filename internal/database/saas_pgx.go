package database

import (
	"context"
	"fmt"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"

	"busca-cnpj-2026/internal/config"
)

// SaaSPool is the pgx v5 pool for opencnpj_saas (API keys, usage, admin).
var SaaSPool *pgxpool.Pool

// InitSaaSPgx opens the SaaS metadata pgx pool when SaaS mode is enabled.
func InitSaaSPgx() error {
	if config.AppConfig == nil || !config.AppConfig.SaaS.Enabled {
		return nil
	}
	url := config.GetSaaSDatabaseURL()
	if url == "" {
		return fmt.Errorf("saas.enabled requires database_saas.url or SAAS_DATABASE_URL")
	}
	cfg, err := pgxpool.ParseConfig(url)
	if err != nil {
		return fmt.Errorf("parse saas dsn: %w", err)
	}
	pool := config.AppConfig.DatabaseSaaS.AsPoolConfig()
	if pool.MaxOpenConns > 0 {
		cfg.MaxConns = int32(pool.MaxOpenConns)
	}
	if pool.MaxIdleConns > 0 {
		cfg.MinConns = int32(pool.MaxIdleConns)
	}
	SaaSPool, err = pgxpool.NewWithConfig(context.Background(), cfg)
	if err != nil {
		return fmt.Errorf("open saas pool: %w", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := SaaSPool.Ping(ctx); err != nil {
		SaaSPool.Close()
		SaaSPool = nil
		return fmt.Errorf("ping saas pool: %w", err)
	}
	return nil
}

// CloseSaaSPgx closes the SaaS pgx pool.
func CloseSaaSPgx() {
	if SaaSPool == nil {
		return
	}
	SaaSPool.Close()
	SaaSPool = nil
}
