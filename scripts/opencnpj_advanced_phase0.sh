#!/usr/bin/env bash
# Plan 02 Phase 0 — advanced baseline gate (prerequisites + plan 01 deliverables).
# Usage: ./scripts/opencnpj_advanced_phase0.sh [API_BASE_URL]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
BENCH="$ROOT/.local/02-opencnpj-advanced-optimization/benchmarks"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PGBOUNCER_CONTAINER="${PGBOUNCER_CONTAINER:-receita-pgbouncer}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"
DATE_TAG="${DATE_TAG:-$(date +%Y%m%d)}"

pass=0
fail=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }

echo "=== OpenCNPJ advanced Phase 0 gate ==="
echo "API: $API_BASE"
echo

echo "--- Delivery gate ---"
if go test ./... -short >/dev/null 2>&1; then ok "go test ./... -short"; else bad "go test ./... -short"; fi
if go vet ./... >/dev/null 2>&1; then ok "go vet ./..."; else bad "go vet ./..."; fi

echo "--- Plan 01 validation (reuse) ---"
if "$ROOT/scripts/api_perf_validation.sh" "$API_BASE" >/tmp/opencnpj-p01-validation.txt 2>&1; then
  ok "api_perf_validation.sh"
else
  bad "api_perf_validation.sh (see /tmp/opencnpj-p01-validation.txt)"
fi

echo "--- pgBouncer ---"
if docker ps --format '{{.Names}}' | grep -qx "$PGBOUNCER_CONTAINER"; then
  ok "pgbouncer container running"
else
  bad "pgbouncer container missing"
fi

echo "--- Partial indexes (000011) ---"
partial=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
  "SELECT count(*) FROM pg_indexes WHERE indexname IN ('idx_estab_nome_fantasia_ativas','idx_estab_cnae_uf_ativas')" 2>/dev/null || echo 0)
if [[ "$partial" == "2" ]]; then ok "partial search indexes"; else bad "partial indexes count=$partial"; fi

echo "--- FTS columns (000012) ---"
fts_cols=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
  "SELECT count(*) FROM information_schema.columns WHERE table_name IN ('empresas','estabelecimentos') AND column_name='busca'" 2>/dev/null || echo 0)
if [[ "$fts_cols" == "2" ]]; then ok "busca tsvector columns"; else bad "busca columns count=$fts_cols"; fi

echo "--- Cache Prometheus metrics ---"
# Warm cache so search paths respond within timeout middleware.
curl -s -o /dev/null --max-time 60 \
  "$API_BASE/api/v1/estabelecimentos/search?nome_fantasia=PADARIA&uf=SP&limit=5" || true
# Force at least one cache miss so miss counter is exported.
curl -s -o /dev/null --max-time 30 \
  "$API_BASE/api/v1/empresas/search?razao_social=ZZZBASELINE$(date +%s)&limit=5" || true

metrics=$(curl -s --max-time 5 "$API_BASE/metrics" 2>/dev/null || true)
if echo "$metrics" | grep -q 'busca_cnpj_cache_hits_total'; then ok "cache_hits_total metric"; else bad "cache_hits_total missing"; fi
if echo "$metrics" | grep -q 'busca_cnpj_cache_misses_total'; then
  ok "cache_misses_total metric"
else
  bad "cache_misses_total missing"
fi

echo "--- UF search smoke ---"
uf_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/estabelecimentos/search?nome_fantasia=PADARIA&uf=SP&limit=5" || true)
if [[ "$uf_code" == "200" ]]; then ok "UF-filtered search -> 200"; else bad "UF search -> $uf_code"; fi

echo
echo "=== Summary: $pass passed, $fail failed ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "Optional: run k6 baseline and system snapshot:"
echo "  $ROOT/scripts/opencnpj_advanced_baseline.sh $API_BASE"
