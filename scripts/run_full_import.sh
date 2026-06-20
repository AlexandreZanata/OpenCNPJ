#!/usr/bin/env bash
# Full 100% import with monitoring and performance report.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DATA="${DATA_PATH:-./data}"
WORKERS="${IMPORT_WORKERS:-8}"
BATCH="${IMPORT_BATCH_SIZE:-100000}"
LOG="/tmp/full_import_100pct.log"
PROGRESS="/tmp/import_progress.log"
REPORT="/tmp/full_import_performance_report.txt"
MONITOR_PID=""

cleanup() {
  if [[ -n "${MONITOR_PID:-}" ]] && kill -0 "$MONITOR_PID" 2>/dev/null; then
    kill "$MONITOR_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "==> Full import started at $(date -Iseconds)"

docker compose up -d postgres
for _ in $(seq 1 60); do
  docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1 && break
  sleep 2
done

echo "==> Drop secondary indexes"
bash scripts/drop_all_import_indexes.sh

echo "==> Truncate fact tables"
docker compose exec -T postgres psql -U receita_user -d receita_db \
  -c "TRUNCATE simples, socios, estabelecimentos, empresas CASCADE;"

rm -f "$PROGRESS" "$REPORT"
INTERVAL_SEC=10 LOG_FILE="$PROGRESS" bash scripts/monitor_import_progress.sh &
MONITOR_PID=$!
echo "==> Monitor PID $MONITOR_PID -> $PROGRESS"

IMPORT_START=$(date +%s.%N)
GOMAXPROCS="$(nproc)" GUARD_ENABLED=true go run ./cmd/importer \
  --data-path="$DATA" \
  --sample-percent=100 \
  --batch-size="$BATCH" \
  --workers="$WORKERS" \
  --tune \
  --skip-refs \
  --benchmark \
  --profile 2>&1 | tee "$LOG"
IMPORT_END=$(date +%s.%N)
IMPORT_WALL=$(python3 -c "print(round($IMPORT_END - $IMPORT_START, 2))")

echo "==> Index rebuild"
INDEX_START=$(date +%s)
docker exec receita-postgres psql -U receita_user -d receita_db -v ON_ERROR_STOP=1 -c "
CREATE INDEX IF NOT EXISTS idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_empresas_natureza_juridica ON empresas(natureza_juridica);
CREATE INDEX IF NOT EXISTS idx_empresas_porte ON empresas(porte_empresa);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnpj_completo ON estabelecimentos(cnpj_completo);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnpj_basico ON estabelecimentos(cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_municipio ON estabelecimentos(municipio);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_uf ON estabelecimentos(uf);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cep ON estabelecimentos(cep);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);
CREATE INDEX IF NOT EXISTS idx_socios_cnpj_basico ON socios(cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_socios_nome_gin ON socios USING gin(nome_socio gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_simples_opcao ON simples(opcao_simples) WHERE opcao_simples = 'S';
CREATE INDEX IF NOT EXISTS idx_simples_mei ON simples(opcao_mei) WHERE opcao_mei = 'S';
ANALYZE empresas; ANALYZE estabelecimentos; ANALYZE socios; ANALYZE simples;
" 2>&1 | tee -a "$LOG"
INDEX_SEC=$(( $(date +%s) - INDEX_START ))

COUNTS=$(docker exec receita-postgres psql -U receita_user -d receita_db -t -A -c "
SELECT 'empresas|'||COUNT(*)||'|'||pg_size_pretty(pg_total_relation_size('empresas')) FROM empresas
UNION ALL SELECT 'estabelecimentos|'||COUNT(*)||'|'||pg_size_pretty(pg_total_relation_size('estabelecimentos')) FROM estabelecimentos
UNION ALL SELECT 'socios|'||COUNT(*)||'|'||pg_size_pretty(pg_total_relation_size('socios')) FROM socios
UNION ALL SELECT 'simples|'||COUNT(*)||'|'||pg_size_pretty(pg_total_relation_size('simples')) FROM simples;
")

BENCHMARK_LINE=$(grep 'BENCHMARK rows=' "$LOG" | tail -1 || true)
PROFILE_LINE=$(grep 'PROFILE wall_sec=' "$LOG" | tail -1 || true)
ROWS=$(echo "$BENCHMARK_LINE" | sed -n 's/.*rows=\([0-9]*\).*/\1/p')
RPS=$(echo "$BENCHMARK_LINE" | sed -n 's/.*rps=\([0-9.]*\).*/\1/p')
TOTAL_WALL=$(python3 -c "print(round($IMPORT_WALL + $INDEX_SEC, 2))")

{
  echo "============================================================"
  echo " FULL IMPORT PERFORMANCE REPORT — $(date -Iseconds)"
  echo "============================================================"
  echo ""
  echo "Configuration"
  echo "  sample_percent: 100"
  echo "  workers:        $WORKERS"
  echo "  batch_size:     $BATCH"
  echo "  postgres:       18.4-alpine"
  echo "  tune:           true"
  echo ""
  echo "Import phase"
  echo "  wall_sec:       $IMPORT_WALL"
  echo "  $BENCHMARK_LINE"
  echo "  $PROFILE_LINE"
  echo ""
  echo "Index rebuild phase"
  echo "  wall_sec:       $INDEX_SEC"
  echo ""
  echo "Total (import + indexes)"
  echo "  wall_sec:       $TOTAL_WALL"
  if [[ -n "$ROWS" && "$ROWS" != "0" ]]; then
    echo "  effective_rps:  $(python3 -c "print(round(int($ROWS)/$IMPORT_WALL))")"
  fi
  echo ""
  echo "Final row counts"
  while IFS='|' read -r table count size; do
    printf "  %-18s %'15s rows  (%s)\n" "$table" "$count" "$size"
  done <<< "$COUNTS"
  echo ""
  echo "Monitor log: $PROGRESS"
  echo "Full log:    $LOG"
  echo ""
  echo "Refreshing analytics aggregates…"
  bash scripts/refresh_stats_aggregates.sh || true
  echo "============================================================"
} | tee "$REPORT"

echo "Report saved: $REPORT"
