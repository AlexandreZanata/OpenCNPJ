#!/usr/bin/env bash
# Plan 02 Phase 3 — Ristretto L1 cache gate (in-process above Redis).
# Usage: ./scripts/opencnpj_advanced_phase3.sh [API_BASE_URL]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
CNPJ_URL="$API_BASE/api/v1/estabelecimentos/33000167000101"

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

echo "=== OpenCNPJ advanced Phase 3 gate (Ristretto L1) ==="
echo "API: $API_BASE"
echo

echo "--- Delivery gate ---"
if go test ./internal/cache/l1/... ./internal/perfvalidation/... -short -run 'Phase3|TestCache' >/dev/null 2>&1; then
  ok "go test L1 + Phase3"
else
  bad "go test L1 + Phase3"
fi
if go test ./internal/services/... -short -run 'L1|Cache' >/dev/null 2>&1; then
  ok "go test services cache L1"
else
  bad "go test services cache L1"
fi
if go test ./internal/perfvalidation/... -short -run 'Phase2' >/dev/null 2>&1; then
  ok "phase 2 artifact tests (Phase2)"
else
  bad "phase 2 artifact tests"
fi

echo "--- Config ---"
if grep -q 'l1_enabled: true' "$ROOT/config/config.yaml"; then
  ok "config cache.l1_enabled"
else
  bad "config cache.l1_enabled missing"
fi

echo "--- L1 Prometheus metrics (warm before heavy k6) ---"
for _ in 1 2 3 4; do
  curl -s -o /dev/null --max-time 30 "$CNPJ_URL" || true
done
metrics=$(curl -s --max-time 5 "$API_BASE/metrics" 2>/dev/null || true)
if echo "$metrics" | grep -q 'busca_cnpj_l1_cache_hits_total'; then
  ok "l1_cache_hits_total metric"
elif echo "$metrics" | grep -q 'busca_cnpj_l1_cache_misses_total'; then
  ok "l1_cache_misses_total metric (L1 active; hits after warm)"
else
  bad "L1 metrics missing (rebuild API: go build -o /tmp/receita-api ./cmd/api)"
fi
if echo "$metrics" | grep -q 'busca_cnpj_l1_cache_misses_total'; then
  ok "l1_cache_misses_total metric"
else
  bad "l1_cache_misses_total missing"
fi

echo "--- CNPJ lookup smoke ---"
code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 "$CNPJ_URL" || true)
if [[ "$code" == "200" ]]; then ok "CNPJ lookup -> 200"; else bad "CNPJ lookup -> $code"; fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi
