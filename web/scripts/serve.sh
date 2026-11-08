#!/bin/sh
# Serves dist/ on http://127.0.0.1:${PORT:-5173} (any static file server works).
set -eu
cd "$(dirname "$0")/.."

[ -d dist ] || sh scripts/build.sh
echo "http://127.0.0.1:${PORT:-5173}"
exec python3 -m http.server "${PORT:-5173}" --bind 127.0.0.1 --directory dist
