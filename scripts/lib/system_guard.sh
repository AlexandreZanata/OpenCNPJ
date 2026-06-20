#!/usr/bin/env bash
# Resource watchdog library — abort or throttle workloads before OOM.
# shellcheck disable=SC2034

GUARD_ROOT="${GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
GUARD_LOG="${GUARD_LOG:-$GUARD_ROOT/data/system_guard.log}"
GUARD_STATE="${GUARD_STATE:-$GUARD_ROOT/data/system_guard.state}"
GUARD_CONF="${GUARD_CONF:-$GUARD_ROOT/scripts/system_guard.conf}"

GUARD_MEM_WARN_PCT="${GUARD_MEM_WARN_PCT:-25}"
GUARD_MEM_THROTTLE_PCT="${GUARD_MEM_THROTTLE_PCT:-12}"
GUARD_MEM_ABORT_PCT="${GUARD_MEM_ABORT_PCT:-5}"
GUARD_MEM_ABORT_MB="${GUARD_MEM_ABORT_MB:-1024}"
GUARD_SWAP_ABORT_PCT="${GUARD_SWAP_ABORT_PCT:-60}"
GUARD_LOAD_THROTTLE_RATIO="${GUARD_LOAD_THROTTLE_RATIO:-1.8}"
GUARD_LOAD_ABORT_RATIO="${GUARD_LOAD_ABORT_RATIO:-3.0}"
GUARD_DISK_ABORT_MB="${GUARD_DISK_ABORT_MB:-2048}"
GUARD_INTERVAL_SEC="${GUARD_INTERVAL_SEC:-3}"
GUARD_THROTTLE_STREAK="${GUARD_THROTTLE_STREAK:-2}"
GUARD_ABORT_STREAK="${GUARD_ABORT_STREAK:-2}"
GUARD_RESUME_STREAK="${GUARD_RESUME_STREAK:-4}"

_guard_load_conf() {
  [[ -f "$GUARD_CONF" ]] || return 0
  # shellcheck disable=SC1090
  source "$GUARD_CONF"
}

_guard_log() {
  mkdir -p "$(dirname "$GUARD_LOG")"
  printf '%s %s\n' "$(date -Iseconds)" "$*" >> "$GUARD_LOG"
  echo "[guard] $*"
}

_guard_collect_metrics() {
  python3 - <<'PY'
import os, shutil

def kb(key):
    with open("/proc/meminfo") as f:
        for line in f:
            if line.startswith(key):
                return int(line.split()[1])
    return 0

total = kb("MemTotal:")
avail = kb("MemAvailable:")
swap_t = kb("SwapTotal:")
swap_f = kb("SwapFree:")
swap_u = swap_t - swap_f
cores = os.cpu_count() or 1
load1 = float(open("/proc/loadavg").read().split()[0])
disk = shutil.disk_usage("/")
avail_mb = avail // 1024
avail_pct = (avail / total * 100) if total else 0
swap_pct = (swap_u / swap_t * 100) if swap_t else 0
load_ratio = load1 / cores
disk_free_mb = disk.free // (1024 * 1024)

print(
    f"avail_mb={avail_mb} avail_pct={avail_pct:.1f} "
    f"swap_pct={swap_pct:.1f} load1={load1:.2f} load_ratio={load_ratio:.2f} "
    f"cores={cores} disk_free_mb={disk_free_mb}"
)
PY
}

_guard_level() {
  local metrics="$1"
  eval "$metrics"

  if (( avail_mb < GUARD_MEM_ABORT_MB )) || \
     (( $(python3 -c "print(1 if $avail_pct < $GUARD_MEM_ABORT_PCT else 0)") )); then
    echo "abort"
    return
  fi
  if (( $(python3 -c "print(1 if $swap_pct >= $GUARD_SWAP_ABORT_PCT and $avail_pct < $GUARD_MEM_THROTTLE_PCT else 0)") )); then
    echo "abort"
    return
  fi
  if (( disk_free_mb < GUARD_DISK_ABORT_MB )); then
    echo "abort"
    return
  fi
  if (( $(python3 -c "print(1 if $load_ratio >= $GUARD_LOAD_ABORT_RATIO and $avail_pct < $GUARD_MEM_WARN_PCT else 0)") )); then
    echo "abort"
    return
  fi

  if (( $(python3 -c "print(1 if $avail_pct < $GUARD_MEM_THROTTLE_PCT else 0)") )) || \
     (( $(python3 -c "print(1 if $load_ratio >= $GUARD_LOAD_THROTTLE_RATIO else 0)") )); then
    echo "throttle"
    return
  fi

  if (( $(python3 -c "print(1 if $avail_pct < $GUARD_MEM_WARN_PCT else 0)") )); then
    echo "warn"
    return
  fi

  echo "normal"
}

guard_status() {
  _guard_load_conf
  local metrics level
  metrics=$(_guard_collect_metrics)
  level=$(_guard_level "$metrics")
  echo "level=$level $metrics"
  echo "config: mem_abort_pct=${GUARD_MEM_ABORT_PCT} mem_abort_mb=${GUARD_MEM_ABORT_MB} swap_abort_pct=${GUARD_SWAP_ABORT_PCT}"
  echo "log: $GUARD_LOG"
}

guard_suggest_workers() {
  local requested="${1:-10}"
  _guard_load_conf
  local metrics avail_mb avail_pct
  metrics=$(_guard_collect_metrics)
  eval "$metrics"

  if (( avail_mb < GUARD_MEM_ABORT_MB )); then
    echo 0
    return
  fi
  if (( avail_mb < 3072 )); then
    echo 2
    return
  fi
  if (( avail_mb < 6144 )); then
    echo $(( requested / 2 > 1 ? requested / 2 : 1 ))
    return
  fi
  if (( $(python3 -c "print(1 if $avail_pct < $GUARD_MEM_WARN_PCT else 0)") )); then
    echo $(( requested * 2 / 3 > 1 ? requested * 2 / 3 : 1 ))
    return
  fi
  echo "$requested"
}

guard_preflight() {
  _guard_load_conf
  local metrics avail_mb avail_pct swap_pct load_ratio
  metrics=$(_guard_collect_metrics)
  eval "$metrics"
  _guard_log "preflight $metrics"

  if (( avail_mb < GUARD_MEM_ABORT_MB )) || \
     (( $(python3 -c "print(1 if $avail_pct < $GUARD_MEM_ABORT_PCT else 0)") )); then
    _guard_log "PREFLIGHT ABORT: memory too low ($metrics)"
    return 1
  fi
  if (( avail_mb < 4096 )) && \
     (( $(python3 -c "print(1 if $swap_pct >= $GUARD_SWAP_ABORT_PCT else 0)") )); then
    _guard_log "PREFLIGHT ABORT: swap high with low RAM ($metrics)"
    return 1
  fi
  return 0
}

_guard_pids_in_pgid() {
  local pgid="$1"
  ps -o pid= --pgid "$pgid" 2>/dev/null | tr -d ' ' | grep -v '^$' || true
}

_guard_throttle_pgid() {
  local pgid="$1"
  local pid
  _guard_log "THROTTLE pgid=$pgid (renice + ionice + SIGSTOP)"
  echo "throttled" > "$GUARD_STATE"
  for pid in $(_guard_pids_in_pgid "$pgid"); do
    renice -n 19 -p "$pid" >/dev/null 2>&1 || true
    ionice -c3 -p "$pid" >/dev/null 2>&1 || true
  done
  for pid in $(_guard_pids_in_pgid "$pgid"); do
    kill -STOP "$pid" 2>/dev/null || true
  done
}

_guard_resume_pgid() {
  local pgid="$1"
  _guard_log "RESUME pgid=$pgid"
  echo "normal" > "$GUARD_STATE"
  local pid
  for pid in $(_guard_pids_in_pgid "$pgid"); do
    kill -CONT "$pid" 2>/dev/null || true
    renice -n 0 -p "$pid" >/dev/null 2>&1 || true
  done
}

_guard_abort_pgid() {
  local pgid="$1" reason="$2"
  _guard_log "ABORT pgid=$pgid reason=$reason"
  echo "abort: $reason" > "$GUARD_STATE"
  kill -TERM -"$pgid" 2>/dev/null || true
  sleep 5
  kill -KILL -"$pgid" 2>/dev/null || true
}

guard_watch_pgid() {
  local pgid="$1"
  _guard_load_conf
  mkdir -p "$(dirname "$GUARD_LOG")" "$(dirname "$GUARD_STATE")"
  echo "watching pgid=$pgid" > "$GUARD_STATE"

  local streak_throttle=0 streak_abort=0 streak_normal=0 throttled=false
  local metrics level

  _guard_log "watch start pgid=$pgid interval=${GUARD_INTERVAL_SEC}s"

  while kill -0 -"$pgid" 2>/dev/null; do
    metrics=$(_guard_collect_metrics)
    level=$(_guard_level "$metrics")

    case "$level" in
      abort)
        streak_abort=$((streak_abort + 1))
        streak_throttle=0
        streak_normal=0
        _guard_log "abort-streak=$streak_abort level=$level $metrics"
        if (( streak_abort >= GUARD_ABORT_STREAK )); then
          _guard_abort_pgid "$pgid" "$metrics"
          return 137
        fi
        ;;
      throttle)
        streak_abort=0
        streak_throttle=$((streak_throttle + 1))
        streak_normal=0
        _guard_log "throttle-streak=$streak_throttle level=$level $metrics"
        if (( streak_throttle >= GUARD_THROTTLE_STREAK )) && [[ "$throttled" == "false" ]]; then
          _guard_throttle_pgid "$pgid"
          throttled=true
        fi
        ;;
      warn)
        streak_abort=0
        streak_throttle=0
        streak_normal=0
        _guard_log "warn $metrics"
        ;;
      normal)
        streak_abort=0
        streak_throttle=0
        streak_normal=$((streak_normal + 1))
        if [[ "$throttled" == "true" ]] && (( streak_normal >= GUARD_RESUME_STREAK )); then
          _guard_resume_pgid "$pgid"
          throttled=false
        fi
        ;;
    esac
    sleep "$GUARD_INTERVAL_SEC"
  done

  echo "finished" > "$GUARD_STATE"
  _guard_log "watch end pgid=$pgid"
  return 0
}
