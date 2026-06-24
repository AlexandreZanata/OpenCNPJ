#!/usr/bin/env bash
set -euo pipefail
exec "$(dirname "$0")/download_latest.sh" "$@"
