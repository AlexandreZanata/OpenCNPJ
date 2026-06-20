#!/usr/bin/env bash
# Poll PostgreSQL row counts and system guard during import.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INTERVAL="${INTERVAL_SEC:-5}"
LOG_FILE="${LOG_FILE:-/tmp/import_progress.log}"

DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

echo "==> Import monitor (every ${INTERVAL}s) — log: ${LOG_FILE}"
echo "started_at=$(date -Iseconds)" | tee -a "$LOG_FILE"

while true; do
  TS="$(date '+%Y-%m-%d %H:%M:%S')"
  COUNTS="$(docker exec receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -t -A -c "
    SELECT 'empresas=' || COUNT(*) FROM empresas
    UNION ALL SELECT 'estabelecimentos=' || COUNT(*) FROM estabelecimentos
    UNION ALL SELECT 'socios=' || COUNT(*) FROM socios
    UNION ALL SELECT 'simples=' || COUNT(*) FROM simples;
  " 2>/dev/null | paste -sd' ' - || echo "db=unavailable")"

  GUARD=""
  if [[ -f "$ROOT/data/system_guard.state" ]]; then
    GUARD="$(tr '\n' ' ' < "$ROOT/data/system_guard.state")"
  fi

  LINE="[$TS] $COUNTS ${GUARD}"
  echo "$LINE" | tee -a "$LOG_FILE"
  sleep "$INTERVAL"
done
