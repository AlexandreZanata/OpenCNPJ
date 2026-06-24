#!/usr/bin/env bash
# Resume a partial full import (keeps empresas/socios, re-imports estabelecimentos + simples).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# shellcheck disable=SC1091
source "$ROOT/scripts/lib/hardware_profile.sh"
hardware_apply_env

WORKERS="${IMPORT_WORKERS:-6}"
BATCH="${IMPORT_BATCH_SIZE:-75000}"
PROGRESS="/tmp/import_progress.log"
RESOURCE_LOG="/tmp/system_resource_monitor.log"
IMPORT_LOG="/tmp/import_resume.log"
STATUS="/tmp/import_resume_status.log"

cleanup() {
  [[ -n "${MONITOR_PID:-}" ]] && kill "$MONITOR_PID" 2>/dev/null || true
  [[ -n "${RESOURCE_PID:-}" ]] && kill "$RESOURCE_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "resume_started_at=$(date -Iseconds)" > "$STATUS"
echo "workers=$WORKERS batch=$BATCH" >> "$STATUS"

INTERVAL_SEC=10 LOG_FILE="$PROGRESS" bash "$ROOT/scripts/monitor_import_progress.sh" &
MONITOR_PID=$!

(
  while true; do
    TS="$(date '+%Y-%m-%d %H:%M:%S')"
    avail="$(awk '/Mem:/ {print $7}' <(free -m))"
    used="$(awk '/Mem:/ {print $3}' <(free -m))"
    swap="$(awk '/Swap:/ {print $3"/"$2}' <(free -m))"
    load="$(awk '{print $1}' /proc/loadavg)"
    guard="$(tr '\n' ' ' < "$ROOT/data/system_guard.state" 2>/dev/null || echo n/a)"
    imp="$(pgrep -cf 'exe/importer|go run.*importer' || true)"
    echo "[$TS] avail_mb=$avail used_mb=$used swap_mb=$swap load=$load guard=$guard importer=$imp" >> "$RESOURCE_LOG"
    sleep 15
  done
) &
RESOURCE_PID=$!

echo "==> Resume import: skip empresas/socios, workers=$WORKERS batch=$BATCH"
echo "    progress: $PROGRESS"
echo "    resources: $RESOURCE_LOG"

bash "$ROOT/scripts/run_with_guard.sh" go run ./cmd/importer \
  --data-path=./data \
  --sample-percent=100 \
  --batch-size="$BATCH" \
  --workers="$WORKERS" \
  --tune \
  --no-clean \
  --skip-refs \
  --skip-empresas \
  --skip-socios \
  --benchmark \
  --profile 2>&1 | tee "$IMPORT_LOG"

rc=${PIPESTATUS[0]}
echo "import_exit=$rc" >> "$STATUS"
echo "import_finished_at=$(date -Iseconds)" >> "$STATUS"
exit "$rc"
