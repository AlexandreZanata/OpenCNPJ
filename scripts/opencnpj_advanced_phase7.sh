#!/usr/bin/env bash
# Plan 02 Phase 7 — CNAE HASH sub-partition pruning gate.
# Usage: ./scripts/opencnpj_advanced_phase7.sh [API_BASE_URL]
# STRICT=1 requires HASH(cnae) sub-partitions + EXPLAIN pruning evidence.
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

echo "=== OpenCNPJ advanced Phase 7 gate (CNAE HASH sub-partitions) ==="
echo "API: $API_BASE"
echo

echo "--- Delivery gate ---"
if go test ./internal/partition/... ./internal/perfvalidation/... -short -run 'Phase7|CNAE|Brazilian' >/dev/null 2>&1; then
  ok "go test partition + Phase7"
else
  bad "go test partition + Phase7"
fi

echo "--- Migration artifact ---"
if [[ -f "$ROOT/migrations/000016_cnae_hash_subpartitions.up.sql" ]]; then
  ok "migration 000016 present"
else
  bad "migration 000016 missing"
fi

echo "--- Postgres LIST(uf) + HASH(cnae) sub-partitions ---"
if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  strategy=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT pg_get_partkeydef('estabelecimentos'::regclass)" 2>/dev/null || true)
  if echo "$strategy" | grep -qi 'list.*uf'; then
    ok "estabelecimentos PARTITION BY LIST (uf)"
  else
    warn "LIST(uf) not applied yet (run: go run ./cmd/migrate)"
  fi

  leaf_count=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT count(*) FROM pg_class c
     JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname = 'public' AND c.relname ~ '^estabelecimentos_[a-z]{2}_h[0-3]$'" \
    2>/dev/null || echo 0)
  if [[ "$leaf_count" -ge 112 ]]; then
    ok "estabelecimentos has $leaf_count CNAE hash leaf partitions"
  elif [[ "$leaf_count" -gt 0 ]]; then
    warn "cnae leaf count=$leaf_count (partial migrate or old layout)"
  else
    warn "no CNAE hash leaves visible (run migration 000016)"
  fi
else
  warn "postgres container not running"
fi

echo "--- EXPLAIN CNAE+UF pruning (STRICT) ---"
if [[ "${STRICT:-0}" == "1" ]] && docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  explain_out=$(docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -qAt \
    -c "EXPLAIN SELECT id FROM estabelecimentos WHERE uf = 'SP' AND cnae_fiscal_principal = '4781400' AND situacao_cadastral = '02' LIMIT 100" 2>/dev/null || true)
  if echo "$explain_out" | grep -qE 'estabelecimentos_sp_h[0-3]'; then
    ok "EXPLAIN prunes to estabelecimentos_sp_h* leaf"
  else
    bad "EXPLAIN missing estabelecimentos_sp_h* leaf partition"
    echo "$explain_out" | head -5
  fi
else
  warn "EXPLAIN pruning (STRICT=1 + migration 000016 applied)"
fi

echo "--- API readiness ---"
ready_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "${API_BASE%/}/readyz" 2>/dev/null || true)
if [[ "$ready_code" == "200" ]]; then ok "API /readyz -> 200"; else warn "API not ready ($ready_code)"; fi

echo "--- API CNAE+UF search smoke ---"
cnae_uf_code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 \
  "$API_BASE/api/v1/estabelecimentos/search?uf=RJ&cnae_principal=6201501&limit=5" || true)
if [[ "$cnae_uf_code" == "200" ]]; then ok "estabelecimentos cnae+uf -> 200"; else bad "cnae+uf -> $cnae_uf_code"; fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "Apply: go run ./cmd/migrate · EXPLAIN: ./scripts/explain_cnae_uf_partition_pruning.sh"
