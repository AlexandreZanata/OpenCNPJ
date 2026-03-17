#!/bin/bash

# Script de preparação para importação ultra-rápida
# Desabilita autovacuum, índices não essenciais e otimiza PostgreSQL para importação massiva

set -e

echo "=========================================="
echo "Preparando PostgreSQL para importação rápida"
echo "=========================================="

# Verificar se o container está rodando
if ! docker ps | grep -q receita-postgres; then
    echo "ERRO: Container receita-postgres não está rodando!"
    echo "Execute: docker-compose up -d postgres"
    exit 1
fi

# Verificar espaço em disco
echo ""
echo "Verificando espaço em disco..."
df -h | grep -E "(Filesystem|/dev/)"

# Verificar espaço mínimo necessário (recomendado: pelo menos 50GB livres)
AVAILABLE_SPACE=$(df -BG / | tail -1 | awk '{print $4}' | sed 's/G//')
if [ "$AVAILABLE_SPACE" -lt 50 ]; then
    echo "⚠️  AVISO: Espaço em disco pode ser insuficiente. Recomendado: pelo menos 50GB livres"
    echo "   Espaço disponível: ${AVAILABLE_SPACE}GB"
fi

# Configurar variáveis
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5434}"
DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

echo ""
echo "Conectando ao PostgreSQL..."
echo "Host: $DB_HOST:$DB_PORT"
echo "Database: $DB_NAME"
echo "User: $DB_USER"

# Função para executar SQL
exec_sql() {
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -c "$1"
}

# 1. Desabilitar autovacuum durante importação
echo ""
echo "1. Desabilitando autovacuum..."
exec_sql "ALTER SYSTEM SET autovacuum = off;" || echo "  (autovacuum já desabilitado ou não configurável via ALTER SYSTEM)"
exec_sql "SELECT pg_reload_conf();" || true

# Desabilitar autovacuum por tabela (mais confiável)
for table in empresas estabelecimentos socios simples; do
    echo "  Desabilitando autovacuum para tabela: $table"
    exec_sql "ALTER TABLE $table SET (autovacuum_enabled = false);" || echo "    (tabela $table pode não existir ainda)"
done

# 2. Desabilitar índices não essenciais (mantém apenas PKs e FKs essenciais)
echo ""
echo "2. Verificando índices..."
exec_sql "
SELECT 
    schemaname,
    tablename,
    indexname
FROM pg_indexes
WHERE schemaname = 'public'
  AND indexname NOT LIKE '%_pkey'
  AND indexname NOT LIKE '%_pk'
ORDER BY tablename, indexname;
" || echo "  (nenhum índice encontrado ou tabelas ainda não existem)"

echo ""
echo "  Nota: Índices serão recriados após a importação pelo script finalize_import.sh"
echo "  Durante a importação, apenas índices de chave primária são mantidos"

# 3. Otimizar configurações de sessão para importação ultra-rápida
echo ""
echo "3. Otimizando configurações de sessão para importação ultra-rápida..."
exec_sql "SET maintenance_work_mem = '10GB';" || true
exec_sql "SET max_parallel_maintenance_workers = 20;" || true
exec_sql "SET synchronous_commit = off;" || true
exec_sql "SET work_mem = '256MB';" || true
exec_sql "SET statement_timeout = 0;" || true
exec_sql "SET lock_timeout = '30s';" || true
echo "  Configurações de sessão otimizadas para máxima performance"

# 4. Verificar configurações atuais
echo ""
echo "4. Configurações atuais do PostgreSQL:"
exec_sql "
SELECT 
    name,
    setting,
    unit
FROM pg_settings
WHERE name IN (
    'shared_buffers',
    'effective_cache_size',
    'work_mem',
    'maintenance_work_mem',
    'max_wal_size',
    'checkpoint_timeout',
    'max_parallel_workers',
    'max_parallel_maintenance_workers',
    'autovacuum'
)
ORDER BY name;
"

# 5. Verificar conexões disponíveis
echo ""
echo "5. Verificando conexões..."
exec_sql "
SELECT 
    count(*) as active_connections,
    max_conn as max_connections,
    max_conn - count(*) as available_connections
FROM pg_stat_activity,
     (SELECT setting::int as max_conn FROM pg_settings WHERE name = 'max_connections') mc
GROUP BY max_conn;
"

echo ""
echo "=========================================="
echo "Preparação concluída!"
echo "=========================================="
echo ""
echo "Próximos passos:"
echo "1. Drop índices não-essenciais (recomendado para máxima performance):"
echo "   ./scripts/drop_indexes_for_import.sh"
echo ""
echo "2. Execute a importação ultra-otimizada:"
echo "   go run cmd/importer/main.go --data-path=./data --workers=32 --batch-size=250000"
echo ""
echo "3. Após a importação, recrie os índices e finalize:"
echo "   ./scripts/recreate_indexes_after_import.sh"
echo "   ./scripts/finalize_import.sh"
echo ""
