#!/usr/bin/env bash
# Detect host RAM/CPU and export recommended import/download settings.
set -euo pipefail

hardware_ram_gb() {
  python3 - <<'PY'
import os
pages = os.sysconf("SC_PHYS_PAGES")
size = os.sysconf("SC_PAGE_SIZE")
print(max(1, (pages * size) // (1024 ** 3)))
PY
}

hardware_cpu_cores() {
  nproc 2>/dev/null || echo 4
}

# Sets IMPORT_WORKERS, IMPORT_BATCH_SIZE, GOMAXPROCS, and prints a summary.
hardware_apply_env() {
  local ram cores
  ram="$(hardware_ram_gb)"
  cores="$(hardware_cpu_cores)"

  if [[ "$ram" -ge 28 ]]; then
    export IMPORT_WORKERS="${IMPORT_WORKERS:-8}"
    export IMPORT_BATCH_SIZE="${IMPORT_BATCH_SIZE:-100000}"
    export GUARD_MEM_WARN_PCT="${GUARD_MEM_WARN_PCT:-25}"
    export GUARD_MEM_THROTTLE_PCT="${GUARD_MEM_THROTTLE_PCT:-18}"
    export PROFILE_NAME="${PROFILE_NAME:-high-ram}"
  elif [[ "$ram" -ge 14 ]]; then
    export IMPORT_WORKERS="${IMPORT_WORKERS:-6}"
    export IMPORT_BATCH_SIZE="${IMPORT_BATCH_SIZE:-75000}"
    export GUARD_MEM_WARN_PCT="${GUARD_MEM_WARN_PCT:-20}"
    export GUARD_MEM_THROTTLE_PCT="${GUARD_MEM_THROTTLE_PCT:-12}"
    export PROFILE_NAME="${PROFILE_NAME:-mid-ram}"
  else
    export IMPORT_WORKERS="${IMPORT_WORKERS:-4}"
    export IMPORT_BATCH_SIZE="${IMPORT_BATCH_SIZE:-50000}"
    export GUARD_MEM_WARN_PCT="${GUARD_MEM_WARN_PCT:-18}"
    export GUARD_MEM_THROTTLE_PCT="${GUARD_MEM_THROTTLE_PCT:-10}"
    export PROFILE_NAME="${PROFILE_NAME:-low-ram}"
  fi

  export GOMAXPROCS="${GOMAXPROCS:-$cores}"

  echo "==> Hardware profile: ${PROFILE_NAME} (${ram} GB RAM, ${cores} CPU cores)"
  echo "    IMPORT_WORKERS=${IMPORT_WORKERS}  IMPORT_BATCH_SIZE=${IMPORT_BATCH_SIZE}  GOMAXPROCS=${GOMAXPROCS}"
}
