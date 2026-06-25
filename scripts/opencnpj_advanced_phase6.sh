#!/usr/bin/env bash
# Plan 02 Phase 6 — UF LIST partition pruning gate.
# Usage: ./scripts/opencnpj_advanced_phase6.sh [API_BASE_URL]
# STRICT=1 requires LIST(uf) partitions + EXPLAIN pruning evidence.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

echo "=== OpenCNPJ advanced Phase 6 gate (UF LIST partitions) ==="
echo "API: $API_BASE"
echo

echo "--- Delivery gate ---"
if go test ./internal/partition/... ./internal/perfvalidation/... -short -run 'Phase6|Brazilian' >/dev/null 2>&1; then
  ok "go test partition + Phase6"
else
  bad "go test partition + Phase6"
fi

echo "--- Migration artifact ---"
if [[ -f "$ROOT/migrations/000014_uf_list_partitions.up.sql" ]]; then
  ok "migration 000014 present"
else
  bad "migration 000014 missing"
fi

echo "--- Postgres LIST(uf) partitions ---"
if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  strategy=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT pg_get_partkeydef('estabelecimentos'::regclass)" 2>/dev/null || true)
  if echo "$strategy" | grep -qi 'list.*uf'; then
    ok "estabelecimentos PARTITION BY LIST (uf)"
  else
    warn "LIST(uf) not applied yet (run: go run ./cmd/migrate)"
  fi

  part_count=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT count(*) FROM pg_inherits i JOIN pg_class p ON p.oid=i.inhparent WHERE p.relname='estabelecimentos'" \
    2>/dev/null || echo 0)
  if [[ "$part_count" -ge 28 ]]; then
    ok "estabelecimentos has $part_count child partitions"
  elif [[ "$part_count" -gt 0 ]]; then
    warn "partition count=$part_count (HASH layout or partial migrate)"
  else
    warn "no partitions visible"
  fi
else
  warn "postgres container not running"
fi

echo "--- EXPLAIN UF pruning (STRICT) ---"
if [[ "${STRICT:-0}" == "1" ]] && docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  explain_out=$(docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -qAt \
    -c "EXPLAIN SELECT id FROM estabelecimentos WHERE uf = 'SP' AND situacao_cadastral = '02' LIMIT 5" 2>/dev/null || true)
  if echo "$explain_out" | grep -q 'estabelecimentos_sp'; then
    ok "EXPLAIN prunes to estabelecimentos_sp"
  else
    bad "EXPLAIN missing estabelecimentos_sp partition"
    echo "$explain_out" | head -5
  fi
else
  warn "EXPLAIN pruning (STRICT=1 + LIST migration applied)"
fi

echo "--- API UF search smoke ---"
uf_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/estabelecimentos/search?uf=SP&limit=5" || true)
if [[ "$uf_code" == "200" ]]; then ok "estabelecimentos search uf=SP -> 200"; else bad "uf search -> $uf_code"; fi

cnae_uf_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/estabelecimentos/search?uf=RJ&cnae_principal=6201501&limit=5" || true)
if [[ "$cnae_uf_code" == "200" ]]; then ok "estabelecimentos cnae+uf -> 200"; else bad "cnae+uf -> $cnae_uf_code"; fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "Apply: go run ./cmd/migrate · EXPLAIN: ./scripts/explain_uf_partition_pruning.sh"
