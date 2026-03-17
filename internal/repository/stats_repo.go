package repository

import (
	"context"
	"database/sql"
	"fmt"

	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/models"
)

type StatsRepository struct {
	db *sql.DB
}

func NewStatsRepository() *StatsRepository {
	return &StatsRepository{
		db: database.DB,
	}
}

func (r *StatsRepository) StatsPerCNAE(ctx context.Context, limit int) ([]models.StatsResponse, error) {
	if limit <= 0 {
		limit = 100
	}

	query := `
		SELECT 
			cnae_fiscal_principal as cnae,
			COUNT(*) as count
		FROM estabelecimentos
		WHERE cnae_fiscal_principal IS NOT NULL
		GROUP BY cnae_fiscal_principal
		ORDER BY count DESC
		LIMIT $1
	`

	rows, err := r.db.QueryContext(ctx, query, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to query stats per CNAE: %w", err)
	}
	defer rows.Close()

	var stats []models.StatsResponse
	for rows.Next() {
		var stat models.StatsResponse
		err := rows.Scan(&stat.CNAE, &stat.Count)
		if err != nil {
			return nil, fmt.Errorf("failed to scan stats: %w", err)
		}
		stats = append(stats, stat)
	}

	return stats, nil
}

func (r *StatsRepository) StatsPerUF(ctx context.Context) ([]models.StatsResponse, error) {
	query := `
		SELECT 
			uf,
			COUNT(*) as count
		FROM estabelecimentos
		WHERE uf IS NOT NULL
		GROUP BY uf
		ORDER BY count DESC
	`

	rows, err := r.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to query stats per UF: %w", err)
	}
	defer rows.Close()

	var stats []models.StatsResponse
	for rows.Next() {
		var stat models.StatsResponse
		err := rows.Scan(&stat.UF, &stat.Count)
		if err != nil {
			return nil, fmt.Errorf("failed to scan stats: %w", err)
		}
		stats = append(stats, stat)
	}

	return stats, nil
}

func (r *StatsRepository) StatsPerCNAEAndUF(ctx context.Context, cnae string, limit int) ([]models.StatsResponse, error) {
	if limit <= 0 {
		limit = 100
	}

	query := `
		SELECT 
			uf,
			COUNT(*) as count
		FROM estabelecimentos
		WHERE cnae_fiscal_principal = $1 AND uf IS NOT NULL
		GROUP BY uf
		ORDER BY count DESC
		LIMIT $2
	`

	rows, err := r.db.QueryContext(ctx, query, cnae, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to query stats per CNAE and UF: %w", err)
	}
	defer rows.Close()

	var stats []models.StatsResponse
	for rows.Next() {
		var stat models.StatsResponse
		err := rows.Scan(&stat.UF, &stat.Count)
		if err != nil {
			return nil, fmt.Errorf("failed to scan stats: %w", err)
		}
		stat.CNAE = cnae
		stats = append(stats, stat)
	}

	return stats, nil
}
