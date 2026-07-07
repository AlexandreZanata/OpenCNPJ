#!/usr/bin/env bash
# Phase 8 — API documentation gate (OpenAPI validate + secret scan + optional live curl).
# Usage: ./scripts/api_docs_gate.sh
# Optional: API_KEY=ocnpj_live_... API_BASE=http://localhost:8080 for live smoke.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPEC="$ROOT/docs/api/OPENAPI.yaml"

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

echo "=== OpenCNPJ API docs gate (Phase 8) ==="
echo

echo "--- Delivery gate ---"
if go test ./internal/apidocs/... ./internal/perfvalidation/... -short -run 'Phase8|Docs|OpenAPI' >/dev/null 2>&1; then
  ok "go test apidocs + Phase8"
else
  bad "go test apidocs + Phase8"
fi

echo "--- Deliverables ---"
for f in OPENAPI.yaml QUICKSTART.md ERRORS.md CHANGELOG.md; do
  if [[ -f "$ROOT/docs/api/$f" ]]; then ok "docs/api/$f"; else bad "missing docs/api/$f"; fi
done

echo "--- OpenAPI validate ---"
if command -v npx >/dev/null 2>&1; then
  if npx --yes @apidevtools/swagger-cli validate "$SPEC" >/dev/null 2>&1; then
    ok "swagger-cli validate"
  else
    bad "swagger-cli validate failed"
    npx --yes @apidevtools/swagger-cli validate "$SPEC" 2>&1 | head -10 || true
  fi
else
  warn "npx not available for swagger-cli"
fi

echo "--- No real API keys in docs ---"
if rg -q 'ocnpj_live_[0-9a-f]{32}' "$ROOT/docs/api" 2>/dev/null; then
  bad "docs/api contains real-looking API key"
else
  ok "no ocnpj_live_<32hex> in docs/api"
fi

echo "--- QUICKSTART live smoke ---"
API_BASE="${API_BASE:-}"
API_KEY="${API_KEY:-}"
if [[ -n "$API_KEY" && -n "$API_BASE" ]]; then
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 15 \
    -H "X-API-Key: ${API_KEY}" "${API_BASE%/}/api/v1/cnpj/00000000000191" || true)
  if [[ "$code" == "200" || "$code" == "404" ]]; then
    ok "QUICKSTART curl -> $code"
  else
    bad "QUICKSTART curl -> $code"
  fi
else
  warn "QUICKSTART live (set API_KEY + API_BASE)"
fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi
