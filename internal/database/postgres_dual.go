package database

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	_ "github.com/lib/pq"

	"busca-cnpj-2026/internal/config"
)

// DBSaaS is the SaaS metadata PostgreSQL pool (api keys, admin, usage).
var DBSaaS *sql.DB

// InitPostgres opens the CNPJ database and, when SaaS mode is on, the SaaS database.
func InitPostgres() error {
	if config.AppConfig != nil && config.AppConfig.SaaS.Enabled {
		return initDualPostgres()
	}
	return initPostgresWithDSN(config.GetDSN(), config.AppConfig.Database.AsPoolConfig())
}

func initDualPostgres() error {
	cnpjURL := config.GetCNPJDatabaseURL()
	if cnpjURL == "" {
		return fmt.Errorf("saas.enabled requires database_cnpj.url or CNPJ_DATABASE_URL")
	}
	saasURL := config.GetSaaSDatabaseURL()
	if saasURL == "" {
		return fmt.Errorf("saas.enabled requires database_saas.url or SAAS_DATABASE_URL")
	}

	var err error
	DB, err = openPostgresPool(cnpjURL, config.AppConfig.DatabaseCNPJ.AsPoolConfig())
	if err != nil {
		return fmt.Errorf("cnpj database: %w", err)
	}

	DBSaaS, err = openPostgresPool(saasURL, config.AppConfig.DatabaseSaaS.AsPoolConfig())
	if err != nil {
		_ = DB.Close()
		DB = nil
		return fmt.Errorf("saas database: %w", err)
	}
	return nil
}

func openPostgresPool(dsn string, pool config.PoolConfig) (*sql.DB, error) {
	db, err := sql.Open("postgres", dsn)
	if err != nil {
		return nil, fmt.Errorf("open: %w", err)
	}
	applyPoolConfig(db, pool)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := db.PingContext(ctx); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("ping: %w", err)
	}
	return db, nil
}

func applyPoolConfig(db *sql.DB, pool config.PoolConfig) {
	maxOpen := pool.MaxOpenConns
	if maxOpen <= 0 {
		maxOpen = 10
	}
	maxIdle := pool.MaxIdleConns
	if maxIdle <= 0 {
		maxIdle = 3
	}
	db.SetMaxOpenConns(maxOpen)
	db.SetMaxIdleConns(maxIdle)
	if config.AppConfig != nil {
		db.SetConnMaxLifetime(time.Duration(config.AppConfig.Database.ConnMaxLifetime) * time.Second)
		db.SetConnMaxIdleTime(time.Duration(config.AppConfig.Database.ConnMaxIdleTime) * time.Second)
	}
}

// InitPostgresForSaasMigrate opens the SaaS metadata database for migrations.
func InitPostgresForSaasMigrate() error {
	url := config.GetSaaSDatabaseURL()
	if url == "" {
		return fmt.Errorf("saas migrate requires database_saas.url or SAAS_DATABASE_URL")
	}
	pool := config.PoolConfig{MaxOpenConns: 2, MaxIdleConns: 1}
	if config.AppConfig != nil {
		pool = config.AppConfig.DatabaseSaaS.AsPoolConfig()
	}
	return initPostgresWithDSN(url, pool)
}

// PostgresReady reports whether required PostgreSQL pools respond to Ping.
func PostgresReady(ctx context.Context) bool {
	if DB == nil || DB.PingContext(ctx) != nil {
		return false
	}
	if config.AppConfig != nil && config.AppConfig.SaaS.Enabled {
		return DBSaaS != nil && DBSaaS.PingContext(ctx) == nil
	}
	return true
}

// ClosePostgres closes all PostgreSQL pools.
func ClosePostgres() error {
	var first error
	if DBSaaS != nil {
		if err := DBSaaS.Close(); err != nil && first == nil {
			first = err
		}
		DBSaaS = nil
	}
	if DB != nil {
		if err := DB.Close(); err != nil && first == nil {
			first = err
		}
		DB = nil
	}
	return first
}
