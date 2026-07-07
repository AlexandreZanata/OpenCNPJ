#!/usr/bin/env bash
# Run EXPLAIN templates for CNAE HASH sub-partition pruning (plan 02 Phase 7).
# Usage: ./scripts/explain_cnae_uf_partition_pruning.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"

if ! docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  echo "postgres container $PG_CONTAINER not running" >&2
  exit 1
fi

docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" \
  -f - < "$ROOT/scripts/explain_cnae_uf_partition_pruning.sql"
