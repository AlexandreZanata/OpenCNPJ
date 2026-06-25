#!/usr/bin/env bash
# Plan 02 Phase 2 — refresh planner stats on search tables.
# Uses analyze-search-tables.sql.example (tracked) or gitignored local *.sql on VPS.
# Usage: ./scripts/vps_analyze_search_tables.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"
SQL_EXAMPLE="$ROOT/deploy/vps/analyze-search-tables.sql.example"
SQL_LOCAL="$ROOT/deploy/vps/analyze-search-tables.sql"

if [[ -n "${VPS_ANALYZE_SQL:-}" ]]; then
  SQL="$VPS_ANALYZE_SQL"
elif [[ -f "$SQL_LOCAL" ]]; then
  SQL="$SQL_LOCAL"
else
  SQL="$SQL_EXAMPLE"
fi

if [[ ! -f "$SQL" ]]; then
  echo "missing SQL file (expected $SQL_EXAMPLE or $SQL_LOCAL)" >&2
  exit 1
fi

echo "=== ANALYZE search tables (plan 02 Phase 2) ==="
echo "SQL: $SQL"
docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -v ON_ERROR_STOP=1 <"$SQL"
echo "=== done ==="
