#!/usr/bin/env bash
# Phase 8 — API performance validation (P0–P2 gate).
# Usage: ./scripts/api_perf_validation.sh [API_BASE_URL]
set -euo pipefail

API_BASE="${1:-http://localhost:8080}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
REDIS_CONTAINER="${REDIS_CONTAINER:-receita-redis}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"

pass=0
fail=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }

echo "=== API performance validation ==="
echo "API: $API_BASE"
echo

echo "--- Go delivery gate ---"
if go test ./... -short >/dev/null 2>&1; then ok "go test ./... -short"; else bad "go test ./... -short"; fi
if go vet ./... >/dev/null 2>&1; then ok "go vet ./..."; else bad "go vet ./..."; fi

echo "--- HTTP smoke ---"
root_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$API_BASE/" || true)
if [[ "$root_code" == "200" ]]; then ok "GET / -> 200"; else bad "GET / -> $root_code"; fi

search_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=5" || true)
if [[ "$search_code" == "200" ]]; then ok "empresas search -> 200"; else bad "empresas search -> $search_code"; fi

gzip_hdr=$(curl -s -D - -o /dev/null -H 'Accept-Encoding: gzip' --max-time 30 \
  "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=5" | grep -i '^content-encoding:' || true)
if echo "$gzip_hdr" | grep -qi gzip; then ok "gzip enabled"; else bad "gzip missing"; fi

echo "--- Keyset pagination ---"
body=$(curl -s --max-time 30 "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=2")
has_more=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin).get('has_more', False))" 2>/dev/null || echo False)
cursor=$(echo "$body" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('next_cursor') or '')" 2>/dev/null || echo "")
if [[ "$has_more" == "True" && -n "$cursor" ]]; then ok "next_cursor present"; else bad "next_cursor missing"; fi

enc_cursor=$(python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))" "$cursor")
p2_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=2&cursor=$enc_cursor" || true)
if [[ "$p2_code" == "200" ]]; then ok "cursor page 2 -> 200"; else bad "cursor page 2 -> $p2_code"; fi

bad_combo=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 \
  "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=2&cursor=cnpj:1&offset=10" || true)
if [[ "$bad_combo" == "400" ]]; then ok "offset+cursor rejected"; else bad "offset+cursor -> $bad_combo"; fi

echo "--- FTS multi-word ---"
fts_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/empresas/search?razao_social=PETRO%20BRAS&limit=5" || true)
if [[ "$fts_code" == "200" ]]; then ok "FTS search -> 200"; else bad "FTS search -> $fts_code"; fi

echo "--- PostgreSQL ---"
if docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
  "SELECT 1 FROM pg_extension WHERE extname='pg_stat_statements'" | grep -q 1; then
  ok "pg_stat_statements enabled"
else
  bad "pg_stat_statements missing"
fi

fts_idx=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
  "SELECT count(*) FROM pg_indexes WHERE indexname IN ('idx_empresas_busca_fts','idx_estabelecimentos_busca_fts')" || echo 0)
if [[ "$fts_idx" == "2" ]]; then ok "FTS GIN indexes present"; else bad "FTS indexes count=$fts_idx"; fi

echo "--- Redis ---"
redis_ping=$(docker exec "$REDIS_CONTAINER" redis-cli ping 2>/dev/null || echo "")
if [[ "$redis_ping" == "PONG" ]]; then ok "redis ping"; else bad "redis ping"; fi

echo "--- Meilisearch (optional) ---"
meili_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 3 http://localhost:7700/health 2>/dev/null || echo "000")
if [[ "$meili_code" == "200" ]]; then ok "meilisearch health"; else echo "[SKIP] meilisearch not running"; fi

echo
echo "=== Summary: $pass passed, $fail failed ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi
