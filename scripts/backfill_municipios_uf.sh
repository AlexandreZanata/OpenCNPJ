#!/usr/bin/env bash
# Populate municipios.uf from estabelecimentos (dominant UF per IBGE municipality code).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

run_psql() {
  if docker ps --format '{{.Names}}' | grep -qx receita-postgres; then
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" "$@"
    return
  fi
  PGPASSWORD="${DB_PASSWORD:-receita_password}" psql \
    -h "${DB_HOST:-localhost}" -p "${DB_PORT:-5434}" \
    -U "$DB_USER" -d "$DB_NAME" "$@"
}

echo "==> Backfilling municipios.uf from estabelecimentos"
start=$(date +%s)
run_psql -v ON_ERROR_STOP=1 <<'SQL'
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
  AND (m.uf IS NULL OR m.uf = '');
ANALYZE municipios;
SQL
elapsed=$(( $(date +%s) - start ))
filled=$(run_psql -t -A -c "SELECT COUNT(*) FROM municipios WHERE uf IS NOT NULL AND uf <> '';")
echo "    municipios with UF: ${filled} (${elapsed}s)"
