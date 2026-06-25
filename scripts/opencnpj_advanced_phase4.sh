#!/usr/bin/env bash
# Plan 02 Phase 4 — materialized views gate (analytics + lookup MVs).
# Usage: ./scripts/opencnpj_advanced_phase4.sh [API_BASE_URL]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

echo "=== OpenCNPJ advanced Phase 4 gate (materialized views) ==="
echo "API: $API_BASE"
echo

echo "--- Delivery gate ---"
if go test ./internal/perfvalidation/... -short -run 'Phase4' >/dev/null 2>&1; then
  ok "go test internal/perfvalidation Phase4"
else
  bad "go test internal/perfvalidation Phase4"
fi
if go test ./internal/repository/... -short >/dev/null 2>&1; then
  ok "go test internal/repository"
else
  bad "go test internal/repository"
fi

echo "--- Migration 000013 applied ---"
if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  mv_count=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT count(*) FROM pg_matviews WHERE matviewname LIKE 'mv_%'" 2>/dev/null || echo 0)
  if [[ "$mv_count" -ge 5 ]]; then ok "postgres has $mv_count materialized views"; else bad "mv count=$mv_count want >=5"; fi
else
  warn "postgres container not running — MV check skipped"
fi

echo "--- Refresh function ---"
if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  fn=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT count(*) FROM pg_proc WHERE proname='refresh_estabelecimento_stats'" 2>/dev/null || echo 0)
  if [[ "$fn" == "1" ]]; then ok "refresh_estabelecimento_stats() exists"; else bad "refresh function missing"; fi
else
  warn "refresh function check skipped"
fi

echo "--- API analytics / lookup smoke ---"
uf_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 "$API_BASE/api/v1/stats/uf" || true)
if [[ "$uf_code" == "200" ]]; then ok "GET /stats/uf -> 200"; else bad "GET /stats/uf -> $uf_code"; fi

summary=$(curl -s --max-time 30 "$API_BASE/api/v1/analytics/summary?cnae_limit=5" 2>/dev/null || true)
if echo "$summary" | grep -q 'materialized_views'; then
  ok "analytics summary source=materialized_views"
else
  bad "analytics summary missing materialized_views source"
fi

lookup_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/lookup/cnae?q=comerc&limit=5" || true)
if [[ "$lookup_code" == "200" ]]; then ok "GET /lookup/cnae -> 200"; else bad "GET /lookup/cnae -> $lookup_code"; fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "Schedule: docs/ops/MATERIALIZED-VIEWS.md (cron refresh after import)"
