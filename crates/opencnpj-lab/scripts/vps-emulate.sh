#!/usr/bin/env bash
# Run a command on one vCPU with a 1 GiB memory cap. The mode used is printed to stderr so
# results can record it; the command's own exit status is returned unchanged.
set -euo pipefail
cpu="${VPS_CPU:-12}"
memory="${VPS_MEMORY_MAX:-1G}"
props=(-p "MemoryMax=$memory" -p MemorySwapMax=0 -p CPUQuota=100%)
if systemd-run --user --scope -q "${props[@]}" true 2>/dev/null; then
  echo "vps-emulate: mode=user-scope cpu=$cpu memory=$memory" >&2
  exec systemd-run --user --scope -q "${props[@]}" taskset -c "$cpu" "$@"
fi
echo "vps-emulate: mode=sudo-scope cpu=$cpu memory=$memory (user scope unavailable)" >&2
exec sudo systemd-run --scope -q "${props[@]}" taskset -c "$cpu" "$@"
