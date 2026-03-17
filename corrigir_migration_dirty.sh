#!/bin/bash

# Script para corrigir migration em estado dirty

set -e

echo "🔧 Corrigindo migration em estado dirty..."

# Limpar estado dirty da migration
docker compose exec -T postgres psql -U receita_user -d receita_db <<EOF
-- Limpar estado dirty
UPDATE schema_migrations SET dirty = false WHERE version = 7;
-- Ou resetar para versão anterior se necessário
-- UPDATE schema_migrations SET version = 6, dirty = false;
EOF

echo "✅ Estado dirty limpo!"
echo ""
echo "Agora você pode executar as migrations novamente:"
echo "  go run cmd/importer/main.go --data-path=./data --workers=32 --batch-size=25000"
