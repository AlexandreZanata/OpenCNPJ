package repository

import (
	"context"
	"fmt"
	"strconv"

	"github.com/lib/pq"

	"busca-cnpj-2026/internal/models"
)

func (r *EstabelecimentoRepository) ListEstabelecimentosByIDs(
	ctx context.Context,
	ids []int64,
) ([]models.EstabelecimentoCompleto, error) {
	if len(ids) == 0 {
		return nil, nil
	}
	query := `SELECT ` + estabelecimentoCompletoSelect + estabelecimentoCompletoFrom + `
		WHERE e.id = ANY($1)`
	rows, err := r.db.QueryContext(ctx, query, pq.Array(ids))
	if err != nil {
		return nil, fmt.Errorf("list estabelecimentos by ids: %w", err)
	}
	defer rows.Close()
	byID := make(map[int64]models.EstabelecimentoCompleto, len(ids))
	for rows.Next() {
		var est models.EstabelecimentoCompleto
		if err := scanEstabelecimentoCompleto(rows, &est); err != nil {
			return nil, err
		}
		byID[est.ID] = est
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	out := make([]models.EstabelecimentoCompleto, 0, len(ids))
	for _, id := range ids {
		if est, ok := byID[id]; ok {
			out = append(out, est)
		}
	}
	return out, nil
}

func ParseEstabIDsFromStrings(ids []string) ([]int64, error) {
	out := make([]int64, 0, len(ids))
	for _, s := range ids {
		id, err := strconv.ParseInt(s, 10, 64)
		if err != nil {
			return nil, fmt.Errorf("parse estab id %q: %w", s, err)
		}
		out = append(out, id)
	}
	return out, nil
}
