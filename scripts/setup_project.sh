#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> Setting up BUSCA-CNPJ-2026"

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "    .env created from .env.example"
else
  echo "    .env already exists"
fi

mkdir -p data
echo "    data/ directory ready"

if command -v docker >/dev/null 2>&1; then
  echo "==> Starting PostgreSQL, Redis, and ClickHouse (docker compose)"
  docker compose up -d postgres redis clickhouse
  echo "    waiting for postgres..."
  for _ in $(seq 1 30); do
    if docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1; then
      echo "    postgres ready on port 5434"
      break
    fi
    sleep 2
  done
else
  echo "    docker not found — skip this step or install Docker"
fi

echo ""
echo "Next steps:"
echo "  1. Download data from Receita Federal:"
echo "       go run ./cmd/downloader"
echo "  2. Import into the database (when cmd/importer is available):"
echo "       go run ./cmd/importer --data-path=./data"
echo "  3. Start the API:"
echo "       go run ./cmd/api"
