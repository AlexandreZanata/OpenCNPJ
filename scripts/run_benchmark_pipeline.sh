#!/usr/bin/env bash
# End-to-end benchmark pipeline: 10% → 20% → index rebuild → API load test.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SAMPLE_10="${SAMPLE_10:-10}"
SAMPLE_20="${SAMPLE_20:-20}"
APPROACH_ID="${APPROACH_ID:-A01}"
WORKERS="${IMPORT_WORKERS:-10}"
BATCH="${IMPORT_BATCH_SIZE:-100000}"
DATA="${DATA_PATH:-./data}"
MONITOR_PID=""

cleanup() {
  if [[ -n "$MONITOR_PID" ]]; then
    kill "$MONITOR_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

start_monitor() {
  bash scripts/monitor_import_progress.sh &
  MONITOR_PID=$!
  echo "==> Monitor PID ${MONITOR_PID} (scripts/monitor_import_progress.sh)"
}

run_sample() {
  local pct="$1"
  echo ""
  echo "=============================================="
  echo " BENCHMARK ${APPROACH_ID} @ ${pct}%"
  echo "=============================================="
  APPROACH_ID="$APPROACH_ID" SAMPLE_PERCENT="$pct" \
    IMPORT_WORKERS="$WORKERS" IMPORT_BATCH_SIZE="$BATCH" \
    IMPORT_TUNE=true DROP_INDEXES=true DATA_PATH="$DATA" \
    bash scripts/benchmark_import_sample.sh
}

echo "==> Step 0: ensure stack + migrations"
docker compose up -d postgres redis
for _ in $(seq 1 60); do
  docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1 && break
  sleep 2
done
go run ./cmd/migrate

start_monitor

echo "==> Step 1: benchmark ${SAMPLE_10}%"
run_sample "$SAMPLE_10"

echo "==> Step 2: benchmark ${SAMPLE_20}%"
run_sample "$SAMPLE_20"

echo "==> Step 3: profile run (${SAMPLE_10}% with --profile)"
bash scripts/drop_all_import_indexes.sh
docker compose exec -T postgres psql -U receita_user -d receita_db \
  -c "TRUNCATE simples, socios, estabelecimentos, empresas CASCADE;"
GOMAXPROCS="$(nproc)" go run ./cmd/importer \
  --data-path="$DATA" \
  --sample-percent="$SAMPLE_10" \
  --batch-size="$BATCH" \
  --workers="$WORKERS" \
  --tune \
  --skip-refs \
  --benchmark \
  --profile 2>&1 | tee "/tmp/benchmark_profile_${SAMPLE_10}pct.log"

echo "==> Step 4: finalize (recreate indexes + VACUUM)"
bash scripts/finalize_import.sh

echo "==> Step 5: API load test"
docker compose up -d api
sleep 5
bash scripts/api_load_test.sh || echo "WARN: load test skipped or failed"

echo ""
echo "==> Results: data/benchmark_comparison.tsv"
column -t -s $'\t' data/benchmark_comparison.tsv 2>/dev/null || cat data/benchmark_comparison.tsv
