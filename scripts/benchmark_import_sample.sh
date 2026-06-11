#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SAMPLE="${SAMPLE_PERCENT:-10}"
WORKERS="${IMPORT_WORKERS:-10}"
BATCH="${IMPORT_BATCH_SIZE:-100000}"
DATA="${DATA_PATH:-./data}"
TARGET_SEC="${TARGET_SEC:-180}"
LOG="/tmp/benchmark_import_${SAMPLE}pct.log"
RESULTS="${ROOT}/data/benchmark_results.tsv"

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
  --benchmark 2>&1 | tee "$LOG"
END=$(date +%s.%N)

ELAPSED=$(python3 -c "print(round($END - $START, 2))")
BENCHMARK_LINE=$(grep '^.*BENCHMARK rows=' "$LOG" | tail -1 || true)

ROWS=0
RPS=0
if [[ -n "$BENCHMARK_LINE" ]]; then
  ROWS=$(echo "$BENCHMARK_LINE" | sed -n 's/.*rows=\([0-9]*\).*/\1/p')
  RPS=$(echo "$BENCHMARK_LINE" | sed -n 's/.*rps=\([0-9.]*\).*/\1/p')
fi

COUNTS=$(docker compose exec -T postgres psql -U receita_user -d receita_db -t -A -c "
SELECT SUM(c) FROM (
  SELECT COUNT(*) c FROM empresas
  UNION ALL SELECT COUNT(*) FROM estabelecimentos
  UNION ALL SELECT COUNT(*) FROM socios
  UNION ALL SELECT COUNT(*) FROM simples
) s;")

mkdir -p "$(dirname "$RESULTS")"
if [[ ! -f "$RESULTS" ]]; then
  echo -e "sample_pct\twall_sec\trows\trps\ttotal_db_rows\trun_at" > "$RESULTS"
fi
echo -e "${SAMPLE}\t${ELAPSED}\t${ROWS}\t${RPS}\t${COUNTS}\t$(date -Iseconds)" >> "$RESULTS"

echo ""
echo "=============================================="
echo " WALL TIME: ${ELAPSED}s (target: ${TARGET_SEC}s)"
echo " ROWS: ${ROWS} | RPS: ${RPS}"
echo " LOG: ${LOG}"
echo "=============================================="

docker compose exec -T postgres psql -U receita_user -d receita_db -c "
SELECT 'empresas' t, COUNT(*) FROM empresas
UNION ALL SELECT 'estabelecimentos', COUNT(*) FROM estabelecimentos
UNION ALL SELECT 'socios', COUNT(*) FROM socios
UNION ALL SELECT 'simples', COUNT(*) FROM simples
ORDER BY 1;
"

if [[ -f "$RESULTS" ]] && [[ $(wc -l < "$RESULTS") -ge 2 ]]; then
  echo ""
  echo "==> Comparison (vs baseline runs in $RESULTS)"
  python3 - "$RESULTS" "$SAMPLE" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
current_pct = float(sys.argv[2])
rows = []
for line in path.read_text().strip().splitlines()[1:]:
    pct, wall, total, rps, db, *_ = line.split("\t")
    rows.append({"pct": float(pct), "wall": float(wall), "rows": int(total), "rps": float(rps)})

cur = next((r for r in rows if r["pct"] == current_pct), None)
base = next((r for r in rows if r["pct"] == 10), None)
if cur and base and cur["pct"] != 10:
    time_ratio = cur["wall"] / base["wall"]
    row_ratio = cur["rows"] / base["rows"] if base["rows"] else 0
    time_pct = (time_ratio - 1) * 100
    row_pct = (row_ratio - 1) * 100
    linearity = (time_ratio / row_ratio * 100) if row_ratio else 0
    print(f"  10% -> {int(current_pct)}%: time +{time_pct:.1f}% ({base['wall']}s -> {cur['wall']}s)")
    print(f"  rows +{row_pct:.1f}% ({base['rows']:,} -> {cur['rows']:,})")
    print(f"  time/rows ratio vs linear: {linearity:.1f}% (100% = perfectly linear)")
PY
fi
