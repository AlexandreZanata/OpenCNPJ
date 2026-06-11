#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> Configurando BUSCA-CNPJ-2026"

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "    .env criado a partir de .env.example"
else
  echo "    .env já existe"
fi

mkdir -p data
echo "    diretório data/ pronto"

if command -v docker >/dev/null 2>&1; then
  echo "==> Subindo PostgreSQL, Redis e ClickHouse (docker compose)"
  docker compose up -d postgres redis clickhouse
  echo "    aguardando postgres..."
  for _ in $(seq 1 30); do
    if docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1; then
      echo "    postgres pronto na porta 5434"
      break
    fi
    sleep 2
  done
else
  echo "    docker não encontrado — pule esta etapa ou instale Docker"
fi

echo ""
echo "Próximos passos:"
echo "  1. Baixar dados da Receita Federal:"
echo "       go run ./cmd/downloader"
echo "  2. Importar para o banco (quando cmd/importer estiver disponível):"
echo "       go run ./cmd/importer --data-path=./data"
echo "  3. Iniciar a API:"
echo "       go run ./cmd/api"
