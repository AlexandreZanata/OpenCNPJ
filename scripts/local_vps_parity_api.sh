#!/usr/bin/env bash
# Start API with VPS parity config (production rate limits, L1 cache, pgBouncer).
# Usage: ./scripts/local_vps_parity_api.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export CONFIG_FILE="${CONFIG_FILE:-$ROOT/config/config.vps-parity.yaml}"
unset BENCHMARK_MODE

if [[ ! -f "$CONFIG_FILE" ]]; then
  echo "missing config: $CONFIG_FILE" >&2
  exit 1
fi

if ! curl -sf --max-time 3 "${API_BASE:-http://localhost:8080}/readyz" >/dev/null 2>&1; then
  echo "Starting API (VPS parity) CONFIG_FILE=$CONFIG_FILE"
  exec go run ./cmd/api
fi

echo "API already listening on :8080 (CONFIG_FILE=$CONFIG_FILE)"
curl -sf "${API_BASE:-http://localhost:8080}/readyz" && echo " ready"
