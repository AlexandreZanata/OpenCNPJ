#!/usr/bin/env bash
set -euo pipefail

go test ./tests/benchmark/... -bench=. -benchmem -benchtime=5s -count=3 \
  | tee "/tmp/bench_$(date +%Y%m%d_%H%M%S).txt"
