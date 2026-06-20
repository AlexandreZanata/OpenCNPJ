#!/usr/bin/env bash
# Lightweight API load test after import (CNPJ lookup, fuzzy search, filtered list).
set -euo pipefail

API_BASE="${API_BASE:-http://localhost:8080}"
DURATION="${LOAD_DURATION:-30s}"
CONCURRENCY="${LOAD_CONCURRENCY:-20}"

pick_cnpj() {
  docker exec receita-postgres psql -U receita_user -d receita_db -t -A -c \
    "SELECT cnpj_completo FROM estabelecimentos WHERE cnpj_completo IS NOT NULL LIMIT 1;" 2>/dev/null \
    | tr -d '[:space:]'
}

CNPJ="$(pick_cnpj)"
if [[ -z "$CNPJ" ]]; then
  echo "WARN: no estabelecimentos row found — skipping load test"
  exit 0
fi

echo "==> API load test base=${API_BASE} cnpj=${CNPJ}"

if command -v hey >/dev/null 2>&1; then
  hey -z "$DURATION" -c "$CONCURRENCY" "${API_BASE}/api/v1/estabelecimentos/search?cnpj_completo=${CNPJ}"
  hey -z "$DURATION" -c "$CONCURRENCY" "${API_BASE}/api/v1/empresas/search?razao_social=LTDA&limit=50"
  hey -z "$DURATION" -c "$CONCURRENCY" \
    "${API_BASE}/api/v1/estabelecimentos/search?uf=SP&cnae_fiscal_principal=6201500&limit=50"
  exit 0
fi

echo "hey not installed — using curl smoke checks"
curl -sf "${API_BASE}/health/live" >/dev/null
curl -sf "${API_BASE}/api/v1/estabelecimentos/search?cnpj_completo=${CNPJ}" | head -c 200
echo ""
curl -sf "${API_BASE}/api/v1/empresas/search?razao_social=LTDA&limit=5" | head -c 200
echo ""
