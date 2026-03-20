package database

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"runtime"
	"sync"
	"time"

	"github.com/golang-migrate/migrate/v4"
	"github.com/golang-migrate/migrate/v4/database/postgres"
	_ "github.com/golang-migrate/migrate/v4/source/file" // Import file source driver for golang-migrate.
	_ "github.com/lib/pq"

	"busca-cnpj-2026/internal/config"
)

var DB *sql.DB

type PreparedStmtPool struct {
	stmts map[string]*sql.Stmt
	mu    sync.RWMutex
}

var StmtPool *PreparedStmtPool

func init() {
	StmtPool = &PreparedStmtPool{
		stmts: make(map[string]*sql.Stmt),
	}
}

func InitPostgres() error {
	dsn := config.GetDSN()

	var err error
	DB, err = sql.Open("postgres", dsn)
	if err != nil {
		return fmt.Errorf("failed to open database: %w", err)
	}

	// Configure connection pool
	maxOpenConns := config.AppConfig.Database.MaxOpenConns
	if maxOpenConns == 0 {
		maxOpenConns = runtime.NumCPU() * 4
	}
	maxIdleConns := config.AppConfig.Database.MaxIdleConns
	if maxIdleConns == 0 {
		maxIdleConns = runtime.NumCPU() * 2
	}

	DB.SetMaxOpenConns(maxOpenConns)
	DB.SetMaxIdleConns(maxIdleConns)
	DB.SetConnMaxLifetime(time.Duration(config.AppConfig.Database.ConnMaxLifetime) * time.Second)
	DB.SetConnMaxIdleTime(time.Duration(config.AppConfig.Database.ConnMaxIdleTime) * time.Second)

	// Test connection
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := DB.PingContext(ctx); err != nil {
		return fmt.Errorf("failed to ping database: %w", err)
	}

	return nil
}

func RunMigrations() error {
	driver, err := postgres.WithInstance(DB, &postgres.Config{})
	if err != nil {
		return fmt.Errorf("failed to create migration driver: %w", err)
	}

	m, err := migrate.NewWithDatabaseInstance(
		"file://migrations",
		"postgres",
		driver,
	)
	if err != nil {
		return fmt.Errorf("failed to create migrate instance: %w", err)
	}

	if err := m.Up(); err != nil && !errors.Is(err, migrate.ErrNoChange) {
		return fmt.Errorf("failed to run migrations: %w", err)
	}

	return nil
}

func ClosePostgres() error {
	if DB != nil {
		return DB.Close()
	}
	return nil
}

// GetPreparedStmt returns a prepared statement from the pool.
func (p *PreparedStmtPool) GetPreparedStmt(name, query string) (*sql.Stmt, error) {
	p.mu.RLock()
	stmt, exists := p.stmts[name]
	p.mu.RUnlock()

	if exists {
		return stmt, nil
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	// Double check
	if stmt, exists := p.stmts[name]; exists {
		return stmt, nil
	}

	//nolint:sqlclosecheck // Statement is cached in the pool and closed by PreparedStmtPool.Close.
	stmt, err := DB.Prepare(query)
	if err != nil {
		return nil, err
	}

	p.stmts[name] = stmt
	return stmt, nil
}

// Close closes all prepared statements.
func (p *PreparedStmtPool) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()

	for _, stmt := range p.stmts {
		if err := stmt.Close(); err != nil {
			return err
		}
	}

	p.stmts = make(map[string]*sql.Stmt)
	return nil
}
