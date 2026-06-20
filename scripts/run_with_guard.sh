#!/usr/bin/env bash
# Wrap any heavy command with the resource watchdog.
# Aborts the entire process group on OOM risk; pauses (SIGSTOP) on throttle.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD_ENABLED="${GUARD_ENABLED:-true}"

if [[ $# -lt 1 ]]; then
  echo "usage: run_with_guard.sh <command...>"
  exit 1
fi

if [[ "$GUARD_ENABLED" != "true" ]]; then
  exec "$@"
fi

# shellcheck source=scripts/lib/system_guard.sh
source "$ROOT/scripts/lib/system_guard.sh"

if ! guard_preflight; then
  echo "[guard] refusing to start — system under memory pressure"
  exit 137
fi

set -m
"$@" &
work_pid=$!
work_pgid=$(ps -o pgid= -p "$work_pid" | tr -d ' ')

bash "$ROOT/scripts/system_guard.sh" daemon --pgid "$work_pgid" &
guard_pid=$!

trap 'kill "$guard_pid" 2>/dev/null || true' EXIT

wait "$work_pid"
exit_code=$?

kill "$guard_pid" 2>/dev/null || true
wait "$guard_pid" 2>/dev/null || true

if [[ -f "$GUARD_STATE" ]] && grep -q '^abort:' "$GUARD_STATE" 2>/dev/null; then
  exit 137
fi

exit "$exit_code"
