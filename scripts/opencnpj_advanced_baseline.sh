#!/usr/bin/env bash
# Plan 02 Phase 0 — capture k6 JSON + system snapshot (local benchmarks).
# Usage: ./scripts/opencnpj_advanced_baseline.sh [API_BASE_URL]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH="$ROOT/.local/02-opencnpj-advanced-optimization/benchmarks"
API_HOST="${1:-http://localhost:8080}"
DATE_TAG="${DATE_TAG:-$(date +%Y%m%d)}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
REDIS_CONTAINER="${REDIS_CONTAINER:-receita-redis}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"

mkdir -p "$BENCH"

# Docker k6 uses host gateway when API runs on host.
K6_API="$API_HOST"
if [[ "$API_HOST" == http://localhost:* ]] || [[ "$API_HOST" == http://127.0.0.1:* ]]; then
  K6_API="${API_HOST/localhost/host.docker.internal}"
  K6_API="${K6_API/127.0.0.1/host.docker.internal}"
fi

echo "Warming cache..."
for u in \
  "$API_HOST/api/v1/estabelecimentos/33000167000101" \
  "$API_HOST/api/v1/empresas/search?razao_social=PETROBRAS&limit=20" \
  "$API_HOST/api/v1/estabelecimentos/search?nome_fantasia=PADARIA&uf=SP&limit=20"; do
  curl -s -o /dev/null "$u" || true
done

echo "=== k6 full baseline ==="
docker run --rm --add-host=host.docker.internal:host-gateway \
  -e API_BASE_URL="$K6_API" \
  -v "$BENCH:/scripts:ro" \
  grafana/k6 run --summary-export "/scripts/k6-advanced-baseline-${DATE_TAG}.json" \
  /scripts/k6-full.js 2>&1 | tee "$BENCH/k6-advanced-baseline-${DATE_TAG}.txt"

echo "=== system snapshot ==="
{
  echo "# system snapshot — $DATE_TAG"
  echo
  echo "## free -h"
  free -h
  echo
  echo "## Postgres table sizes"
  docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -c \
    "SELECT relname, pg_size_pretty(pg_total_relation_size(relid)) AS total
     FROM pg_catalog.pg_statio_user_tables
     WHERE relname IN ('empresas','estabelecimentos')
     ORDER BY pg_total_relation_size(relid) DESC;"
  echo
  echo "## Top indexes (estabelecimentos)"
  docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -c \
    "SELECT indexrelname, pg_size_pretty(pg_relation_size(indexrelid)) AS size
     FROM pg_stat_user_indexes
     WHERE relname = 'estabelecimentos'
     ORDER BY pg_relation_size(indexrelid) DESC
     LIMIT 10;"
  echo
  echo "## Redis memory"
  docker exec "$REDIS_CONTAINER" redis-cli INFO memory 2>/dev/null | grep -E 'used_memory_human|maxmemory_human' || true
} | tee "$BENCH/system-snapshot.txt"

if [[ -f "$BENCH/explain-partition-pruning.sql" ]]; then
  echo "=== EXPLAIN partition templates ==="
  docker cp "$BENCH/explain-partition-pruning.sql" "$PG_CONTAINER:/tmp/explain-partition-pruning.sql"
  docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" \
    -f /tmp/explain-partition-pruning.sql > "$BENCH/explain-partition-pruning-${DATE_TAG}.txt" 2>&1 || true
fi

echo "Done. Artifacts in $BENCH/"
