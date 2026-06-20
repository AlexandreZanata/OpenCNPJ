package repository

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"time"

	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/models"
)

const (
	tableStatsByUF     = "stats_estabelecimentos_by_uf"
	tableStatsByCNAE   = "stats_estabelecimentos_by_cnae"
	tableStatsByCNAEUF = "stats_estabelecimentos_by_cnae_uf"
)

// ErrStatsNotReady indicates aggregate tables were not refreshed after import.
var ErrStatsNotReady = errors.New("stats aggregates empty — run refresh_estabelecimento_stats() after import")

type StatsRepository struct {
	db *sql.DB
}

func NewStatsRepository() *StatsRepository {
	return &StatsRepository{db: database.DB}
}

func (r *StatsRepository) StatsPerUF(ctx context.Context) ([]models.StatsResponse, error) {
	query := fmt.Sprintf(`
		SELECT uf, count FROM %s
		ORDER BY count DESC, uf ASC
	`, tableStatsByUF)

	return r.scanUFStats(ctx, query)
}

func (r *StatsRepository) StatsPerCNAE(ctx context.Context, limit int) ([]models.StatsResponse, error) {
	if limit <= 0 {
		limit = 100
	}

	query := fmt.Sprintf(`
		SELECT cnae, count FROM %s
		ORDER BY count DESC, cnae ASC
		LIMIT $1
	`, tableStatsByCNAE)

	rows, err := r.db.QueryContext(ctx, query, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to query stats per CNAE: %w", err)
	}
	defer rows.Close()

	return scanCNAEStats(rows)
}

func (r *StatsRepository) StatsPerCNAEAndUF(
	ctx context.Context,
	cnae string,
	limit int,
) ([]models.StatsResponse, error) {
	if limit <= 0 {
		limit = 100
	}

	query := fmt.Sprintf(`
		SELECT uf, count FROM %s
		WHERE cnae = $1
		ORDER BY count DESC, uf ASC
		LIMIT $2
	`, tableStatsByCNAEUF)

	rows, err := r.db.QueryContext(ctx, query, cnae, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to query stats per CNAE and UF: %w", err)
	}
	defer rows.Close()

	stats, err := r.scanUFStatsRows(rows)
	if err != nil {
		return nil, err
	}

	for i := range stats {
		stats[i].CNAE = cnae
	}

	return stats, nil
}

func (r *StatsRepository) AnalyticsSummary(
	ctx context.Context,
	cnaeLimit int,
	cnaeUFLimit int,
) (*models.AnalyticsSummaryResponse, error) {
	if cnaeLimit <= 0 {
		cnaeLimit = 15
	}
	if cnaeUFLimit <= 0 {
		cnaeUFLimit = 10
	}

	byUF, err := r.StatsPerUF(ctx)
	if err != nil {
		return nil, err
	}
	if len(byUF) == 0 {
		return nil, ErrStatsNotReady
	}

	topCNAE, err := r.StatsPerCNAE(ctx, cnaeLimit)
	if err != nil {
		return nil, err
	}
	if len(topCNAE) == 0 {
		return nil, ErrStatsNotReady
	}

	topCNAEUF, err := r.StatsPerCNAEAndUF(ctx, topCNAE[0].CNAE, cnaeUFLimit)
	if err != nil {
		return nil, err
	}

	refreshedAt, err := r.lastRefreshedAt(ctx)
	if err != nil {
		return nil, err
	}

	return &models.AnalyticsSummaryResponse{
		Source:      "aggregates",
		RefreshedAt: refreshedAt,
		ByUF:        byUF,
		TopCNAE:     topCNAE,
		TopCNAEUF: models.CNAEUFBreakdown{
			CNAE: topCNAE[0].CNAE,
			ByUF: topCNAEUF,
		},
	}, nil
}

func (r *StatsRepository) lastRefreshedAt(ctx context.Context) (string, error) {
	query := fmt.Sprintf(`SELECT MAX(refreshed_at) FROM %s`, tableStatsByUF)
	var ts sql.NullTime
	if err := r.db.QueryRowContext(ctx, query).Scan(&ts); err != nil {
		return "", fmt.Errorf("failed to read stats refresh timestamp: %w", err)
	}
	if !ts.Valid {
		return "", nil
	}
	return ts.Time.UTC().Format(time.RFC3339), nil
}

func (r *StatsRepository) scanUFStats(ctx context.Context, query string) ([]models.StatsResponse, error) {
	rows, err := r.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to query stats per UF: %w", err)
	}
	defer rows.Close()

	return r.scanUFStatsRows(rows)
}

func (r *StatsRepository) scanUFStatsRows(rows *sql.Rows) ([]models.StatsResponse, error) {
	var stats []models.StatsResponse
	for rows.Next() {
		var stat models.StatsResponse
		if err := rows.Scan(&stat.UF, &stat.Count); err != nil {
			return nil, fmt.Errorf("failed to scan stats: %w", err)
		}
		stats = append(stats, stat)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed iterating stats rows: %w", err)
	}
	return stats, nil
}

func scanCNAEStats(rows *sql.Rows) ([]models.StatsResponse, error) {
	var stats []models.StatsResponse
	for rows.Next() {
		var stat models.StatsResponse
		if err := rows.Scan(&stat.CNAE, &stat.Count); err != nil {
			return nil, fmt.Errorf("failed to scan stats: %w", err)
		}
		stats = append(stats, stat)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed iterating stats rows: %w", err)
	}
	return stats, nil
}
