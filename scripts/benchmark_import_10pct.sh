#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SAMPLE="${SAMPLE_PERCENT:-10}"
WORKERS="${IMPORT_WORKERS:-10}"
BATCH="${IMPORT_BATCH_SIZE:-100000}"
DATA="${DATA_PATH:-./data}"
TARGET_SEC="${TARGET_SEC:-180}"

echo "=============================================="
echo " BENCHMARK: ${SAMPLE}% import (target: ${TARGET_SEC}s)"
echo " workers=${WORKERS} batch=${BATCH}"
echo "=============================================="

bash scripts/drop_all_import_indexes.sh

echo "==> Truncate fact tables"
docker compose exec -T postgres psql -U receita_user -d receita_db \
  -c "TRUNCATE simples, socios, estabelecimentos, empresas CASCADE;"

START=$(date +%s.%N)
GOMAXPROCS="$(nproc)" go run ./cmd/importer \
  --data-path="$DATA" \
  --sample-percent="$SAMPLE" \
  --batch-size="$BATCH" \
  --workers="$WORKERS" \
  --tune \
  --skip-refs \
  --benchmark 2>&1 | tee /tmp/benchmark_import_10pct.log
END=$(date +%s.%N)

ELAPSED=$(python3 -c "print(round($END - $START, 2))")
echo ""
echo "=============================================="
echo " WALL TIME: ${ELAPSED}s (target: ${TARGET_SEC}s)"
echo "=============================================="

docker compose exec -T postgres psql -U receita_user -d receita_db -c "
SELECT 'empresas' t, COUNT(*) FROM empresas
UNION ALL SELECT 'estabelecimentos', COUNT(*) FROM estabelecimentos
UNION ALL SELECT 'socios', COUNT(*) FROM socios
UNION ALL SELECT 'simples', COUNT(*) FROM simples
ORDER BY 1;
"

if python3 -c "exit(0 if float('$ELAPSED') <= float('$TARGET_SEC') else 1)"; then
  echo "PASS: within ${TARGET_SEC}s target"
else
  echo "MISS: above ${TARGET_SEC}s target — see /tmp/benchmark_import_10pct.log"
fi
