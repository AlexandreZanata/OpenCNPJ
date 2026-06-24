package importer

import (
	"context"
	"fmt"

	"github.com/jackc/pgx/v5/pgxpool"
)

// TruncateMainTables clears fact tables before a fresh import.
func TruncateMainTables(ctx context.Context, pool *pgxpool.Pool) error {
	_, err := pool.Exec(ctx, `TRUNCATE simples, socios, estabelecimentos, empresas CASCADE`)
	if err != nil {
		return fmt.Errorf("truncate all: %w", err)
	}
	return nil
}

func TruncateChildTables(ctx context.Context, pool *pgxpool.Pool) error {
	_, err := pool.Exec(ctx, `TRUNCATE simples, socios, estabelecimentos CASCADE`)
	if err != nil {
		return fmt.Errorf("truncate children: %w", err)
	}
	return nil
}

func TruncateSociosSimples(ctx context.Context, pool *pgxpool.Pool) error {
	_, err := pool.Exec(ctx, `TRUNCATE simples, socios CASCADE`)
	if err != nil {
		return fmt.Errorf("truncate socios/simples: %w", err)
	}
	return nil
}
