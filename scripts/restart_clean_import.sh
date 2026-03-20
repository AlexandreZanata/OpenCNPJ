#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${PROJECT_DIR}"

DATA_PATH="${DATA_PATH:-./data}"
WORKERS="${WORKERS:-16}"
BATCH_SIZE="${BATCH_SIZE:-250000}"
WIPE_VOLUMES="${WIPE_VOLUMES:-false}"

echo "==> Projeto: ${PROJECT_DIR}"
echo "==> DATA_PATH: ${DATA_PATH}"
echo "==> WORKERS: ${WORKERS}"
echo "==> BATCH_SIZE: ${BATCH_SIZE}"
echo "==> WIPE_VOLUMES: ${WIPE_VOLUMES}"

if [[ ! -d "${DATA_PATH}" ]]; then
  echo "ERRO: pasta de dados nao existe: ${DATA_PATH}"
  exit 1
fi

echo "==> Parando processos de importacao em execucao (se houver)"
pkill -f "/bin/importer" || true
pkill -f "cmd/importer/main.go" || true

if [[ "${WIPE_VOLUMES}" == "true" ]]; then
  echo "==> Recriando banco limpo (down -v)"
  docker compose down -v --remove-orphans
fi

echo "==> Subindo infraestrutura (postgres, redis, clickhouse)"
docker compose up -d postgres redis clickhouse

echo "==> Aguardando PostgreSQL ficar pronto..."
for i in {1..60}; do
  if docker compose exec -T postgres pg_isready -U receita_user -d receita_db >/dev/null 2>&1; then
    echo "==> PostgreSQL pronto"
    break
  fi

  if [[ "$i" -eq 60 ]]; then
    echo "ERRO: PostgreSQL nao ficou pronto a tempo"
    exit 1
  fi

  sleep 1
done

echo "==> Build do importer"
make build

echo "==> Iniciando nova importacao com banco limpo (clean default)"
./bin/importer --data-path="${DATA_PATH}" --workers="${WORKERS}" --batch-size="${BATCH_SIZE}"

echo "==> Importacao finalizada"
