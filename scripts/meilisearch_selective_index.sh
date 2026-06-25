#!/usr/bin/env bash
# Selective Meilisearch index — active matriz only (plan 02 Phase 5).
# Requires: meilisearch.enabled=true, Meilisearch on :7700, Postgres with data.
# Usage: ./scripts/meilisearch_selective_index.sh [--max-batches N]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MAX_BATCHES=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --max-batches) MAX_BATCHES="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

EXTRA_ARGS=()
if [[ -n "$MAX_BATCHES" ]]; then
  EXTRA_ARGS+=("-max-batches" "$MAX_BATCHES")
fi

echo "=== Meilisearch selective index (active matriz) ==="
go run ./cmd/meilisearch-index "${EXTRA_ARGS[@]}"
echo "=== done ==="
