#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/lib/system_guard.sh
source "$ROOT/scripts/lib/system_guard.sh"

usage() {
  cat <<'EOF'
Usage:
  system_guard.sh status
  system_guard.sh preflight
  system_guard.sh suggest-workers [N]
  system_guard.sh watch-pgid <pgid>
  system_guard.sh daemon [--pgid PGID | --pid PID]

Environment:
  GUARD_ENABLED=true   Enable in benchmark scripts (default: true)
  See scripts/system_guard.conf for thresholds.
EOF
}

cmd="${1:-status}"
shift || true

case "$cmd" in
  status)
    guard_status
    ;;
  preflight)
    guard_preflight
    ;;
  suggest-workers)
    guard_suggest_workers "${1:-10}"
    ;;
  watch-pgid)
    guard_watch_pgid "${1:?pgid required}"
    ;;
  daemon)
    pgid=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --pgid) pgid="$2"; shift 2 ;;
        --pid)
          pgid=$(ps -o pgid= -p "$2" 2>/dev/null | tr -d ' ')
          shift 2
          ;;
        *) echo "unknown: $1"; usage; exit 1 ;;
      esac
    done
    [[ -n "$pgid" ]] || { echo "daemon requires --pgid or --pid"; exit 1; }
    guard_watch_pgid "$pgid"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "unknown command: $cmd"
    usage
    exit 1
    ;;
esac
