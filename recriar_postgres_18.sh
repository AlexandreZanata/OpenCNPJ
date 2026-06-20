#!/bin/bash

# Script para recriar PostgreSQL 18.4 e aplicar migrations

set -e

echo "🛑 Parando containers..."
docker compose down postgres

echo "🗑️  Removendo volume do PostgreSQL (para forçar recriação com PostgreSQL 18.4)..."
docker volume rm busca-cnpj-2026_postgres_data 2>/dev/null || echo "Volume não existe ou já foi removido"

echo "🚀 Subindo PostgreSQL 18.4..."
docker compose up -d postgres

echo "⏳ Aguardando PostgreSQL 18.4 estar pronto..."
sleep 5

# Aguardar até o PostgreSQL estar realmente pronto
until docker compose exec -T postgres pg_isready -U receita_user -d receita_db > /dev/null 2>&1; do
    echo "Aguardando PostgreSQL..."
    sleep 2
done

echo "✅ PostgreSQL 18.4 está pronto!"
echo ""
echo "📊 Verificando versão do PostgreSQL:"
docker compose exec -T postgres psql -U receita_user -d receita_db -c "SELECT version();"

echo ""
echo "🔧 Agora execute as migrations e importação:"
echo ""
echo "1. Rodar migrations:"
echo "   go run cmd/api/main.go"
echo ""
echo "2. Reimportar dados:"
echo "   go run cmd/importer/main.go --data-path=./data --workers=32 --batch-size=25000"
echo ""
