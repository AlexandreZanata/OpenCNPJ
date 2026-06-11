#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SAMPLE="${SAMPLE_PERCENT:-10}"
DATA="${DATA_PATH:-./data}"
BATCH="${IMPORT_BATCH_SIZE:-10000}"

echo "==> Starting Postgres stack"
docker compose up -d postgres redis

echo "==> Waiting for Postgres"
for _ in $(seq 1 60); do
  if docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

echo "==> Migrations"
go run ./cmd/migrate

echo "==> Import ${SAMPLE}% sample"
go run ./cmd/importer \
  --data-path="$DATA" \
  --sample-percent="$SAMPLE" \
  --batch-size="$BATCH"

echo "==> Benchmark fixtures"
bash scripts/seed_test_fixtures.sh "$DATA" ./tests/fixtures 10000

echo "==> Row counts"
docker compose exec -T postgres psql -U receita_user -d receita_db -c "
SELECT 'empresas' AS t, COUNT(*) FROM empresas
UNION ALL SELECT 'estabelecimentos', COUNT(*) FROM estabelecimentos
UNION ALL SELECT 'socios', COUNT(*) FROM socios
UNION ALL SELECT 'simples', COUNT(*) FROM simples
ORDER BY 1;
"
