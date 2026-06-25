#!/usr/bin/env bash
# Plan 02 Phase 2 — PostgreSQL 16 GB production profile gate.
# Usage: ./scripts/opencnpj_advanced_phase2.sh [API_BASE_URL]
# STRICT_VPS=1 enforces live SHOW GUC values on Postgres (direct port / container).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
STRICT_VPS="${STRICT_VPS:-0}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"
BENCH="$ROOT/.local/02-opencnpj-advanced-optimization/benchmarks"
K6_SCRIPT="$BENCH/k6-full.js"

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

pg_show() {
  docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc "SHOW $1;" 2>/dev/null | tr -d '[:space:]' || true
}

normalize_guc() {
  local v="${1,,}"
  v="${v// /}"
  echo "$v"
}

echo "=== OpenCNPJ advanced Phase 2 gate (PostgreSQL 16 GB profile) ==="
echo "API: $API_BASE  STRICT_VPS=$STRICT_VPS"
echo

echo "--- Phase 1 prerequisite ---"
if "$ROOT/scripts/opencnpj_advanced_phase1.sh" "$API_BASE" >/tmp/opencnpj-p2-p1.txt 2>&1; then
  ok "phase 1 gate passed"
else
  bad "phase 1 gate failed (see /tmp/opencnpj-p2-p1.txt)"
fi

echo "--- Delivery gate ---"
if go test ./internal/perfvalidation/... -short -run 'Phase2' >/dev/null 2>&1; then
  ok "go test internal/perfvalidation Phase2"
else
  bad "go test internal/perfvalidation Phase2"
fi

echo "--- Deploy artifacts ---"
for f in \
  deploy/vps/postgresql-opencnpj.conf.example \
  deploy/vps/postgresql-autovacuum-opencnpj.conf.example \
  deploy/vps/analyze-search-tables.sql.example \
  docs/ops/VPS-POSTGRESQL.md \
  scripts/vps_analyze_search_tables.sh \
  scripts/vps_apply_postgresql_conf.sh; do
  if [[ -f "$ROOT/$f" ]]; then ok "artifact $f"; else bad "missing $f"; fi
done

pg_main="$ROOT/deploy/vps/postgresql-opencnpj.conf.example"
if grep -q 'shared_buffers = 4GB' "$pg_main" && grep -q 'work_mem = 64MB' "$pg_main"; then
  ok "postgresql template has shared_buffers + work_mem"
else
  bad "postgresql template memory GUCs incomplete"
fi

if ! grep -qiE 'autovacuum=off|full_page_writes=off|wal_level=minimal|fsync=off' "$pg_main"; then
  ok "postgresql template excludes import-dev flags"
else
  bad "postgresql template contains forbidden import-dev flags"
fi

if grep -q 'autovacuum = on' "$ROOT/deploy/vps/postgresql-autovacuum-opencnpj.conf.example"; then
  ok "autovacuum enabled in include file"
else
  bad "autovacuum include missing"
fi

echo "--- ANALYZE script (functional) ---"
if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  if "$ROOT/scripts/vps_analyze_search_tables.sh" >/tmp/opencnpj-p2-analyze.txt 2>&1; then
    ok "vps_analyze_search_tables.sh"
  else
    bad "vps_analyze_search_tables.sh (see /tmp/opencnpj-p2-analyze.txt)"
  fi
else
  warn "postgres container not running — ANALYZE script skipped"
fi

echo "--- Live GUC check (STRICT_VPS=$STRICT_VPS) ---"
if [[ "$STRICT_VPS" == "1" ]]; then
  if ! docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
    bad "postgres container required for STRICT_VPS GUC check"
  else
    declare -A wants=(
      [shared_buffers]=4gb
      [effective_cache_size]=12gb
      [work_mem]=64mb
      [maintenance_work_mem]=2gb
      [autovacuum]=on
      [wal_level]=replica
      [full_page_writes]=on
    )
    for key in "${!wants[@]}"; do
      got=$(normalize_guc "$(pg_show "$key")")
      want="${wants[$key]}"
      if [[ "$got" == "$want" ]]; then
        ok "SHOW $key=$got"
      else
        bad "SHOW $key=$got want $want (copy and apply deploy/vps/*.example on VPS)"
      fi
    done
  fi
else
  warn "live GUC checks (set STRICT_VPS=1 after VPS profile apply)"
fi

echo "--- Light k6 smoke ---"
k6_ok=0
K6_API="$API_BASE"
if [[ "$API_BASE" == http://localhost:* ]] || [[ "$API_BASE" == http://127.0.0.1:* ]]; then
  K6_API="${API_BASE/localhost/host.docker.internal}"
  K6_API="${K6_API/127.0.0.1/host.docker.internal}"
fi
if [[ -f "$K6_SCRIPT" ]] && command -v docker >/dev/null 2>&1; then
  for u in \
    "$API_BASE/api/v1/estabelecimentos/33000167000101" \
    "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=20"; do
    curl -s -o /dev/null --max-time 15 "$u" || true
  done
  docker run --rm --add-host=host.docker.internal:host-gateway \
    -e API_BASE_URL="$K6_API" \
    -v "$K6_SCRIPT:/scripts/k6-full.js:ro" \
    grafana/k6 run --vus 2 --duration 10s /scripts/k6-full.js >/tmp/opencnpj-p2-k6.txt 2>&1 || true
  k6_fail_rate=$(grep 'http_req_failed' /tmp/opencnpj-p2-k6.txt 2>/dev/null | tail -1 | sed -n 's/.*: \([0-9.]*\)%.*/\1/p')
  if [[ -n "$k6_fail_rate" ]] && awk -v r="$k6_fail_rate" 'BEGIN{exit !(r+0 < 1)}'; then
    ok "light k6 (${k6_fail_rate}% errors)"
    k6_ok=1
  else
    warn "light k6 inconclusive (see /tmp/opencnpj-p2-k6.txt)"
  fi
else
  warn "k6 smoke skipped"
fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "On VPS after PG apply: STRICT_VPS=1 $0 $API_BASE"
