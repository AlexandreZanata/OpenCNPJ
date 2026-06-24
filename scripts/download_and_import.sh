#!/usr/bin/env bash
# One command: download latest data → migrate → full import with live performance logs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# shellcheck disable=SC1091
source "$ROOT/scripts/lib/hardware_profile.sh"
hardware_apply_env

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  CNPJ Full pipeline — download + import + performance log   ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "  Step 1/3  Download latest Receita Federal data"
echo "  Step 2/3  Apply database migrations"
echo "  Step 3/3  Import 100% + index rebuild + performance report"
echo ""
echo "  Estimated time (32 GB RAM): ~20–25 min import + ~30–60 min download (network)"
echo "  Reports: /tmp/full_import_performance_report.txt"
echo ""

bash "$ROOT/scripts/download_latest.sh"

echo ""
echo "==> Step 2/3: migrations"
docker compose up -d postgres
for _ in $(seq 1 60); do
  docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1 && break
  sleep 2
done
go run ./cmd/migrate

echo ""
echo "==> Step 3/3: full import (live rows/s every 10s below)"
echo ""

IMPORT_METRICS_INTERVAL="${IMPORT_METRICS_INTERVAL:-10}" \
  bash "$ROOT/scripts/run_full_import.sh"

echo ""
echo "==> Pipeline complete. Start the API: go run ./cmd/api"
