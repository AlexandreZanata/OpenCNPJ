#!/usr/bin/env bash
# Post-deploy smoke test for OpenCNPJ SaaS API.
#
# Usage:
#   ./scripts/saas_smoke.sh BASE_URL [API_KEY]
#
# Examples:
#   ./scripts/saas_smoke.sh https://api.comerc.app.br
#   ./scripts/saas_smoke.sh http://127.0.0.1:8081 "$TEST_API_KEY"
set -euo pipefail

BASE_URL="${1:?BASE_URL required (e.g. https://api.comerc.app.br)}"
API_KEY="${2:-}"
CNPJ_TEST="00000000000191"

BASE_URL="${BASE_URL%/}"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

check_http() {
  local method="$1" path="$2" want="$3" extra_args=("${@:4}")
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' -X "$method" "${extra_args[@]}" "${BASE_URL}${path}" || true)"
  if [[ "$code" != "$want" ]]; then
    fail "$method $path => $code (want $want)"
  fi
  echo "OK: $method $path => $want"
}

echo "==> Smoke test: $BASE_URL"

check_http GET "/livez" "200"
check_http GET "/readyz" "200"
check_http GET "/api/v1/cnpj/${CNPJ_TEST}" "401"

if [[ -n "$API_KEY" ]]; then
  code="$(curl -s -o /dev/null -w '%{http_code}' \
    -H "X-API-Key: ${API_KEY}" "${BASE_URL}/api/v1/cnpj/${CNPJ_TEST}" || true)"
  if [[ "$code" != "200" && "$code" != "404" ]]; then
    fail "authenticated CNPJ lookup => $code (want 200 or 404)"
  fi
  echo "OK: GET /api/v1/cnpj/{cnpj} with API key => $code"
else
  echo "SKIP: authenticated lookup (no API_KEY arg)"
fi

echo "✓ SaaS smoke test passed"
