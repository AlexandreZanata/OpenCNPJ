#!/usr/bin/env bash
set -euo pipefail
export SAMPLE_PERCENT="${SAMPLE_PERCENT:-10}"
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/benchmark_import_sample.sh"
