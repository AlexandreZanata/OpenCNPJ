#!/usr/bin/env bash
# Refresh pre-aggregated estabelecimento statistics (run after full import).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PGHOST="${PGHOST:-localhost}"
PGPORT="${PGPORT:-5434}"
PGUSER="${PGUSER:-receita_user}"
PGDATABASE="${PGDATABASE:-receita_db}"
export PGPASSWORD="${PGPASSWORD:-receita_password}"

echo "Refreshing materialized views (analytics + lookup) on ${PGHOST}:${PGPORT}/${PGDATABASE}…"
START=$(date +%s)

docker exec receita-postgres psql -U "$PGUSER" -d "$PGDATABASE" -c \
  "SELECT * FROM refresh_estabelecimento_stats();"

ELAPSED=$(( $(date +%s) - START ))
echo "Stats refresh completed in ${ELAPSED}s"
