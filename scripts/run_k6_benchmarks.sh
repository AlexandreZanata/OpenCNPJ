#!/usr/bin/env bash
# Run k6 benchmarks via Docker (no local k6 install required).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH="$ROOT/.local/01-api-performance-optimization/benchmarks"
API_BASE_URL="${API_BASE_URL:-http://host.docker.internal:8080}"
K6="docker run --rm --add-host=host.docker.internal:host-gateway -e API_BASE_URL=$API_BASE_URL -v $BENCH:/scripts:ro grafana/k6"

echo "Warming cache..."
# Restart API with BENCHMARK_MODE if reachable via docker api container; host API: export BENCHMARK_MODE=true
export BENCHMARK_MODE="${BENCHMARK_MODE:-true}"
for u in \
  "$API_BASE_URL/api/v1/estabelecimentos/33000167000101" \
  "$API_BASE_URL/api/v1/empresas/search?razao_social=PETROBRAS&limit=20" \
  "$API_BASE_URL/api/v1/estabelecimentos/search?nome_fantasia=PADARIA&limit=20"; do
  curl -s -o /dev/null "$u" || true
done

echo "=== k6 baseline (10 VU, 30s, warm cache) ==="
$K6 run /scripts/k6-baseline.js 2>&1 | tee "$BENCH/k6-baseline-results.txt"

echo "=== k6 50 VU gate (CNPJ lookup) ==="
$K6 run /scripts/k6-50vu.js 2>&1 | tee "$BENCH/k6-50vu-results.txt"

echo "=== k6 keyset deep pages ==="
$K6 run /scripts/k6-keyset-deep.js 2>&1 | tee "$BENCH/k6-keyset-deep-results.txt"

echo "Done. Results in $BENCH/"
