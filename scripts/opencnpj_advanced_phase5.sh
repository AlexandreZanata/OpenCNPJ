#!/usr/bin/env bash
# Plan 02 Phase 5 — Meilisearch selective index gate (active matriz ~20M target).
# Usage: ./scripts/opencnpj_advanced_phase5.sh [API_BASE_URL]
# MEILI_STRICT=1 runs sample index when Meilisearch is healthy.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
MEILI_HOST="${MEILI_HOST:-localhost}"
MEILI_PORT="${MEILI_PORT:-7700}"
MEILI_KEY="${MEILI_MASTER_KEY:-dev_master_key_change_me}"

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

echo "=== OpenCNPJ advanced Phase 5 gate (Meilisearch selective) ==="
echo "API: $API_BASE"
echo

echo "--- Delivery gate ---"
if go test ./internal/meilisearch/... ./internal/perfvalidation/... -short -run 'Phase5|Selective|Health' >/dev/null 2>&1; then
  ok "go test meilisearch + Phase5"
else
  bad "go test meilisearch + Phase5"
fi
if go test ./internal/config/... -short -run Meilisearch >/dev/null 2>&1; then
  ok "go test config meilisearch defaults"
else
  bad "go test config meilisearch"
fi

echo "--- Config ---"
if grep -q 'selective_active_matriz: true' "$ROOT/config/config.yaml"; then
  ok "config selective_active_matriz"
else
  bad "config selective_active_matriz missing"
fi

echo "--- Meilisearch health ---"
meili_health=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 \
  -H "Authorization: Bearer $MEILI_KEY" "http://${MEILI_HOST}:${MEILI_PORT}/health" 2>/dev/null || true)
if [[ "$meili_health" == "200" ]]; then
  ok "meilisearch /health -> 200"
else
  warn "meilisearch not running (optional for local gate)"
fi

echo "--- Postgres search fallback smoke ---"
search_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=5" || true)
if [[ "$search_code" == "200" ]]; then ok "empresas search -> 200"; else bad "empresas search -> $search_code"; fi

echo "--- Optional selective sample index ---"
if [[ "${MEILI_STRICT:-0}" == "1" && "$meili_health" == "200" ]]; then
  if go run "$ROOT/cmd/meilisearch-index" -batch-size 200 -max-batches 1 >/tmp/opencnpj-p5-index.txt 2>&1; then
    ok "selective sample index (1 batch)"
  else
    bad "selective sample index failed (see /tmp/opencnpj-p5-index.txt)"
  fi
else
  warn "sample index (MEILI_STRICT=1 + healthy Meilisearch)"
fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "Full index: enable meilisearch in config + ./scripts/meilisearch_selective_index.sh"
