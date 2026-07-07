#!/usr/bin/env bash
# Build stripped OpenCNPJ API binary for VPS deployment.
# Usage: ./scripts/build_opencnpj_api.sh [OUTPUT_PATH]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-/usr/local/bin/opencnpj-api}"

cd "$ROOT"
echo "==> Building $OUT"
go build -ldflags="-s -w" -o "$OUT" ./cmd/api
echo "OK: $(file "$OUT" 2>/dev/null || ls -lh "$OUT")"
