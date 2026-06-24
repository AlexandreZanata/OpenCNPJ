package repository

import (
	"context"
	"fmt"
)

// backfillMunicipiosUF sets municipios.uf from the dominant UF per IBGE code in estabelecimentos.
// RFB MUNICCSV has no UF column; run after estabelecimentos import.
const backfillMunicipiosUFSQL = `
WITH mode_uf AS (
	SELECT municipio, uf,
	       ROW_NUMBER() OVER (PARTITION BY municipio ORDER BY COUNT(*) DESC, uf) AS rn
	FROM estabelecimentos
	WHERE NULLIF(TRIM(municipio), '') IS NOT NULL
	  AND NULLIF(TRIM(uf), '') IS NOT NULL
	GROUP BY municipio, uf
)
UPDATE municipios m
SET uf = mode_uf.uf
FROM mode_uf
WHERE m.codigo = mode_uf.municipio
  AND mode_uf.rn = 1
  AND (m.uf IS NULL OR m.uf = '')`

// BackfillMunicipiosUF populates municipios.uf for fast city lookup without scanning estabelecimentos.
func (r *LookupRepository) BackfillMunicipiosUF(ctx context.Context) (int64, error) {
	res, err := r.db.ExecContext(ctx, backfillMunicipiosUFSQL)
	if err != nil {
		return 0, fmt.Errorf("backfill municipios uf: %w", err)
	}
	return res.RowsAffected()
}
