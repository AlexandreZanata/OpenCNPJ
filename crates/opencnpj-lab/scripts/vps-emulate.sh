#!/usr/bin/env bash
# Run a command on one vCPU with a 1 GiB memory cap.
set -euo pipefail
cpu="${VPS_CPU:-12}"
memory="${VPS_MEMORY_MAX:-1G}"
if systemd-run --user --scope -q -p MemoryMax="$memory" -p MemorySwapMax=0 -p CPUQuota=100% \
  taskset -c "$cpu" "$@"; then
  exit 0
fi
exec sudo systemd-run --scope -q -p MemoryMax="$memory" -p MemorySwapMax=0 -p CPUQuota=100% \
  taskset -c "$cpu" "$@"
