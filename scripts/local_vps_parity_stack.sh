#!/usr/bin/env bash
# Local stack matching VPS API conditions: clean import → VPS PG profile → benchmark.
#
# Usage:
#   ./scripts/local_vps_parity_stack.sh              # CLEAN=1 full pipeline
#   SKIP_IMPORT=1 ./scripts/local_vps_parity_stack.sh   # reuse DB, apply VPS PG + benchmark
#   START_API=1 ./scripts/local_vps_parity_stack.sh       # also start API in foreground at end
#
# Phases:
#   1. Clean volume (optional) + import with fast Postgres (base compose)
#   2. Recreate Postgres with VPS production GUCs (docker-compose.vps-parity.yml)
#   3. ANALYZE + refresh materialized views
#   4. Frontend-oriented benchmark → docs/benchmarks/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CLEAN="${CLEAN:-1}"
SKIP_IMPORT="${SKIP_IMPORT:-0}"
START_API="${START_API:-0}"
COMPOSE=(docker compose -f docker-compose.yml)
COMPOSE_VPS=(docker compose -f docker-compose.yml -f docker-compose.vps-parity.yml)

wait_pg() {
  for _ in $(seq 1 90); do
    docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1 && return 0
    sleep 2
  done
  echo "postgres not ready" >&2
  exit 1
}

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Local VPS API parity — clean import + production PG profile ║"
echo "╚══════════════════════════════════════════════════════════════╝"

if [[ "$CLEAN" == "1" && "$SKIP_IMPORT" != "1" ]]; then
  echo "==> Phase 0: wipe volumes (clean database)"
  docker compose down -v
fi

if [[ "$SKIP_IMPORT" != "1" ]]; then
  echo "==> Phase 1: import stack (fast Postgres tuning)"
  "${COMPOSE[@]}" up -d postgres pgbouncer redis
  wait_pg
  go run ./cmd/migrate
  echo "==> Phase 1b: full import (100%) — expect ~20–40 min on 32 GB RAM"
  bash "$ROOT/scripts/run_full_import.sh"
fi

echo "==> Phase 2: switch Postgres to VPS 16 GB production profile"
"${COMPOSE_VPS[@]}" up -d postgres --force-recreate
wait_pg

echo "==> Phase 3: planner stats + materialized views"
bash "$ROOT/scripts/vps_analyze_search_tables.sh"
bash "$ROOT/scripts/refresh_stats_aggregates.sh"

echo "==> Phase 4: verify VPS GUCs"
docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SELECT name, setting FROM pg_settings WHERE name IN ('shared_buffers','work_mem','autovacuum','wal_level') ORDER BY 1"

# Start API in background if not running
if ! curl -sf --max-time 3 http://localhost:8080/readyz >/dev/null 2>&1; then
  echo "==> Starting API (background) with VPS parity config"
  CONFIG_FILE="$ROOT/config/config.vps-parity.yaml" \
    nohup go run ./cmd/api > /tmp/vps-parity-api.log 2>&1 &
  for _ in $(seq 1 60); do
    curl -sf --max-time 2 http://localhost:8080/readyz >/dev/null 2>&1 && break
    sleep 2
  done
fi

echo "==> Phase 5: frontend benchmark"
bash "$ROOT/scripts/local_vps_frontend_benchmark.sh" http://localhost:8080

echo ""
echo "Done."
echo "  API log:    /tmp/vps-parity-api.log"
echo "  Web UI:     make web-dev  →  http://localhost:5173"
echo "  API:        http://localhost:8080  (CONFIG_FILE=config/config.vps-parity.yaml)"
echo "  Benchmark:  docs/benchmarks/$(date +%Y-%m-%d)-vps-parity-local-frontend.md"

if [[ "$START_API" == "1" ]]; then
  exec env CONFIG_FILE="$ROOT/config/config.vps-parity.yaml" go run ./cmd/api
fi
