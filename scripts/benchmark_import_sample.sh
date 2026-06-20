#!/usr/bin/env bash
set -euo pipefail
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GUARD_ENABLED="${GUARD_ENABLED:-true}"
APPROACH_ID="${APPROACH_ID:-A01}"
SAMPLE="${SAMPLE_PERCENT:-10}"
WORKERS="${IMPORT_WORKERS:-10}"
BATCH="${IMPORT_BATCH_SIZE:-100000}"
TUNE="${IMPORT_TUNE:-true}"
DROP_INDEXES="${DROP_INDEXES:-true}"
DATA="${DATA_PATH:-./data}"
TARGET_SEC="${TARGET_SEC:-180}"
LOG="/tmp/benchmark_${APPROACH_ID}_${SAMPLE}pct.log"
RESULTS="${ROOT}/data/benchmark_comparison.tsv"

TUNE_ARGS=()
[[ "$TUNE" == "true" ]] && TUNE_ARGS+=(--tune)

if [[ "$GUARD_ENABLED" == "true" ]]; then
  # shellcheck source=scripts/lib/system_guard.sh
  source "$ROOT/scripts/lib/system_guard.sh"
  if ! guard_preflight; then
    echo "[guard] aborted before benchmark — insufficient memory"
    exit 137
  fi
  SUGGESTED=$(guard_suggest_workers "$WORKERS")
  if [[ "$SUGGESTED" == "0" ]]; then
    echo "[guard] aborted — cannot run with safe worker count"
    exit 137
  fi
  if [[ "$SUGGESTED" != "$WORKERS" ]]; then
    echo "[guard] reducing workers ${WORKERS} -> ${SUGGESTED} (memory pressure)"
    WORKERS="$SUGGESTED"
  fi
fi

echo "=============================================="
echo " APPROACH ${APPROACH_ID} | SAMPLE ${SAMPLE}%"
echo " workers=${WORKERS} batch=${BATCH} tune=${TUNE} drop_idx=${DROP_INDEXES}"
echo " guard=${GUARD_ENABLED}"
echo "=============================================="

if [[ "$DROP_INDEXES" == "true" ]]; then
  bash scripts/drop_all_import_indexes.sh
fi

echo "==> Truncate fact tables"
docker compose exec -T postgres psql -U receita_user -d receita_db \
  -c "TRUNCATE simples, socios, estabelecimentos, empresas CASCADE;"

GUARD_PID=""
if [[ "$GUARD_ENABLED" == "true" ]]; then
  bash "$ROOT/scripts/system_guard.sh" daemon --pid $$ &
  GUARD_PID=$!
  trap 'kill "$GUARD_PID" 2>/dev/null || true' EXIT
fi

START=$(date +%s.%N)
GOMAXPROCS="$(nproc)" go run ./cmd/importer \
  --data-path="$DATA" \
  --sample-percent="$SAMPLE" \
  --batch-size="$BATCH" \
  --workers="$WORKERS" \
  "${TUNE_ARGS[@]}" \
  --skip-refs \
  --benchmark 2>&1 | tee "$LOG" || IMPORT_EXIT=$?
IMPORT_EXIT=${IMPORT_EXIT:-0}

if [[ -n "$GUARD_PID" ]]; then
  kill "$GUARD_PID" 2>/dev/null || true
  wait "$GUARD_PID" 2>/dev/null || true
  if [[ -f "${GUARD_STATE:-$ROOT/data/system_guard.state}" ]] && \
     grep -q '^abort:' "${GUARD_STATE:-$ROOT/data/system_guard.state}" 2>/dev/null; then
    echo "[guard] benchmark aborted to protect system"
    exit 137
  fi
fi

[[ "$IMPORT_EXIT" -eq 0 ]] || exit "$IMPORT_EXIT"

END=$(date +%s.%N)

ELAPSED=$(python3 -c "print(round($END - $START, 2))")
BENCHMARK_LINE=$(grep 'BENCHMARK rows=' "$LOG" | tail -1 || true)
ROWS=0
RPS=0
if [[ -n "$BENCHMARK_LINE" ]]; then
  ROWS=$(echo "$BENCHMARK_LINE" | sed -n 's/.*rows=\([0-9]*\).*/\1/p')
  RPS=$(echo "$BENCHMARK_LINE" | sed -n 's/.*rps=\([0-9.]*\).*/\1/p')
fi

mkdir -p "$(dirname "$RESULTS")"
if [[ ! -f "$RESULTS" ]]; then
  echo -e "approach_id\tsample_pct\twall_sec\trows\trps\trun_at" > "$RESULTS"
fi
echo -e "${APPROACH_ID}\t${SAMPLE}\t${ELAPSED}\t${ROWS}\t${RPS}\t$(date -Iseconds)" >> "$RESULTS"

echo ""
echo "RESULT approach=${APPROACH_ID} sample=${SAMPLE}% wall=${ELAPSED}s rows=${ROWS} rps=${RPS}"
