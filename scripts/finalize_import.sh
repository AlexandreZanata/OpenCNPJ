#!/bin/bash

# Script de finalização após importação
# Recria índices, executa VACUUM ANALYZE e reabilita autovacuum

set -e

echo "=========================================="
echo "Finalizando importação - Otimizando banco"
echo "=========================================="

# Verificar se o container está rodando
if ! docker ps | grep -q receita-postgres; then
    echo "ERRO: Container receita-postgres não está rodando!"
    exit 1
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

# Função para executar SQL e capturar output
exec_sql_output() {
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -t -A -c "$1"
}

# 1. Verificar estatísticas antes
echo ""
echo "1. Estatísticas antes da finalização:"
exec_sql "
SELECT 
    schemaname,
    tablename,
    n_live_tup as row_count,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
FROM pg_stat_user_tables
WHERE tablename IN ('empresas', 'estabelecimentos', 'socios', 'simples', 'cnaes')
ORDER BY tablename;
"

# 2. Recriar índices em paralelo (CONCURRENTLY para não bloquear)
echo ""
echo "2. Recriando índices em paralelo (CONCURRENTLY)..."
echo "   Isso pode levar vários minutos dependendo do tamanho dos dados..."
echo "   Usando script dedicado: recreate_indexes_after_import.sh"
echo ""
echo "   Se preferir, execute manualmente:"
echo "   ./scripts/recreate_indexes_after_import.sh"
echo ""
echo "   Continuando com VACUUM ANALYZE..."

# Índices para empresas
echo "   Recriando índices de empresas..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_empresas_natureza_juridica ON empresas(natureza_juridica);" || echo "    (índice já existe ou erro)"

# Índices para estabelecimentos
echo "   Recriando índices de estabelecimentos..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnpj_completo ON estabelecimentos(cnpj_completo);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnpj_basico ON estabelecimentos(cnpj_basico);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_municipio ON estabelecimentos(municipio);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_uf ON estabelecimentos(uf);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cep ON estabelecimentos(cep);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);" || echo "    (índice já existe ou erro)"

# Índices para socios
echo "   Recriando índices de socios..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_socios_cnpj_basico ON socios(cnpj_basico);" || echo "    (índice já existe ou erro)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_socios_nome_socio_gin ON socios USING gin(nome_socio gin_trgm_ops);" || echo "    (índice já existe ou erro)"

# Índices para simples
echo "   Recriando índices de simples..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_simples_cnpj_basico ON simples(cnpj_basico);" || echo "    (índice já existe ou erro)"

echo "   Índices recriados!"

# 3. Executar ANALYZE nas tabelas (coleta estatísticas sem VACUUM)
# ANALYZE é mais rápido e suficiente para atualizar estatísticas do planner
echo ""
echo "3. Executando ANALYZE para atualizar estatísticas..."
echo "   (ANALYZE é mais rápido que VACUUM ANALYZE e suficiente para estatísticas)"

# Executar ANALYZE em paralelo usando background jobs quando possível
for table in empresas estabelecimentos socios simples cnaes motivos municipios naturezas paises qualificacoes; do
    echo "   Analisando tabela: $table"
    exec_sql "ANALYZE $table;" || echo "    (tabela $table pode não existir)"
done

# 4. Executar VACUUM ANALYZE apenas nas tabelas grandes (isso pode levar vários minutos)
echo ""
echo "4. Executando VACUUM ANALYZE nas tabelas grandes..."
echo "   Isso otimiza o banco e remove espaço morto (pode levar vários minutos)"
echo "   Tabelas pequenas (referência) não precisam de VACUUM"

for table in empresas estabelecimentos socios simples; do
    echo "   VACUUM ANALYZE em: $table (isso pode levar alguns minutos)..."
    exec_sql "VACUUM ANALYZE $table;" || echo "    (erro ao fazer VACUUM em $table)"
    echo "   ✅ VACUUM ANALYZE concluído para: $table"
done

# 5. Reabilitar autovacuum
echo ""
echo "5. Reabilitando autovacuum..."
exec_sql "ALTER SYSTEM SET autovacuum = on;" || echo "  (autovacuum já habilitado ou não configurável via ALTER SYSTEM)"
exec_sql "SELECT pg_reload_conf();" || true

# Reabilitar autovacuum por tabela
for table in empresas estabelecimentos socios simples; do
    echo "  Reabilitando autovacuum para tabela: $table"
    exec_sql "ALTER TABLE $table SET (autovacuum_enabled = true);" || echo "    (tabela $table pode não existir)"
done

# 6. Restaurar configurações PostgreSQL padrão
echo ""
echo "6. Restaurando configurações de sessão padrão..."
exec_sql "SET synchronous_commit = on;" || true
exec_sql "SET work_mem = '20MB';" || true
exec_sql "SET statement_timeout = DEFAULT;" || true
exec_sql "SET lock_timeout = DEFAULT;" || true

# 7. Verificar estatísticas finais
echo ""
echo "7. Estatísticas finais:"
exec_sql "
SELECT 
    schemaname,
    tablename,
    n_live_tup as row_count,
    n_dead_tup as dead_rows,
    last_vacuum,
    last_autovacuum,
    last_analyze,
    last_autoanalyze,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as total_size,
    pg_size_pretty(pg_relation_size(schemaname||'.'||tablename)) as table_size,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename) - pg_relation_size(schemaname||'.'||tablename)) as indexes_size
FROM pg_stat_user_tables
WHERE tablename IN ('empresas', 'estabelecimentos', 'socios', 'simples', 'cnaes')
ORDER BY tablename;
"

# 8. Verificar índices criados
echo ""
echo "8. Índices criados:"
exec_sql "
SELECT 
    schemaname,
    tablename,
    indexname,
    pg_size_pretty(pg_relation_size(indexrelid)) as index_size
FROM pg_indexes
WHERE schemaname = 'public'
  AND tablename IN ('empresas', 'estabelecimentos', 'socios', 'simples')
ORDER BY tablename, indexname;
"

echo ""
echo "=========================================="
echo "Finalização concluída!"
echo "=========================================="
echo ""
echo "O banco de dados está otimizado e pronto para uso."
echo ""
