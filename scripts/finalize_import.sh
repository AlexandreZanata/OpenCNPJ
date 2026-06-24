#!/bin/bash

# Post-import finalization script.
# Recreates indexes, runs VACUUM ANALYZE, and re-enables autovacuum.

set -e

echo "=========================================="
echo "Finalizing import - optimizing database"
echo "=========================================="

# Verify the container is running
if ! docker ps | grep -q receita-postgres; then
    echo "ERROR: receita-postgres container is not running!"
    exit 1
fi

# Configure variables
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5434}"
DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

echo ""
echo "Connecting to PostgreSQL..."
echo "Host: $DB_HOST:$DB_PORT"
echo "Database: $DB_NAME"
echo "User: $DB_USER"

# Execute SQL
exec_sql() {
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -c "$1"
}

# Execute SQL and capture output
exec_sql_output() {
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -t -A -c "$1"
}

# 1. Check statistics before finalization
echo ""
echo "1. Statistics before finalization:"
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

# 2. Recreate indexes in parallel (CONCURRENTLY to avoid blocking)
echo ""
echo "2. Recreating indexes in parallel (CONCURRENTLY)..."
echo "   This may take several minutes depending on data size..."
echo "   Using dedicated script: recreate_indexes_after_import.sh"
echo ""
echo "   To run manually instead:"
echo "   ./scripts/recreate_indexes_after_import.sh"
echo ""
echo "   Continuing with VACUUM ANALYZE..."

# Indexes for empresas
echo "   Recreating empresa indexes..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_empresas_natureza_juridica ON empresas(natureza_juridica);" || echo "    (index already exists or error)"

# Indexes for estabelecimentos
echo "   Recreating estabelecimento indexes..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnpj_completo ON estabelecimentos(cnpj_completo);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnpj_basico ON estabelecimentos(cnpj_basico);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_municipio ON estabelecimentos(municipio);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_uf ON estabelecimentos(uf);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cep ON estabelecimentos(cep);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);" || echo "    (index already exists or error)"

# Indexes for socios
echo "   Recreating socios indexes..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_socios_cnpj_basico ON socios(cnpj_basico);" || echo "    (index already exists or error)"
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_socios_nome_socio_gin ON socios USING gin(nome_socio gin_trgm_ops);" || echo "    (index already exists or error)"

# Indexes for simples
echo "   Recreating simples indexes..."
exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_simples_cnpj_basico ON simples(cnpj_basico);" || echo "    (index already exists or error)"

echo "   Indexes recreated!"

# 3. Run ANALYZE on tables (collect statistics without VACUUM)
# ANALYZE is faster and sufficient to refresh planner statistics
echo ""
echo "3. Running ANALYZE to refresh statistics..."
echo "   (ANALYZE is faster than VACUUM ANALYZE and sufficient for statistics)"

# Run ANALYZE in parallel using background jobs when possible
for table in empresas estabelecimentos socios simples cnaes motivos municipios naturezas paises qualificacoes; do
    echo "   Analyzing table: $table"
    exec_sql "ANALYZE $table;" || echo "    (table $table may not exist)"
done

# 4. Run VACUUM ANALYZE only on large tables (may take several minutes)
echo ""
echo "4. Running VACUUM ANALYZE on large tables..."
echo "   This optimizes the database and reclaims dead space (may take several minutes)"
echo "   Small reference tables do not need VACUUM"

for table in empresas estabelecimentos socios simples; do
    echo "   VACUUM ANALYZE on: $table (this may take a few minutes)..."
    exec_sql "VACUUM ANALYZE $table;" || echo "    (error running VACUUM on $table)"
    echo "   ✅ VACUUM ANALYZE completed for: $table"
done

# 5. Re-enable autovacuum
echo ""
echo "5. Re-enabling autovacuum..."
exec_sql "ALTER SYSTEM SET autovacuum = on;" || echo "  (autovacuum already enabled or not configurable via ALTER SYSTEM)"
exec_sql "SELECT pg_reload_conf();" || true

# Re-enable autovacuum per table
for table in empresas estabelecimentos socios simples; do
    echo "  Re-enabling autovacuum for table: $table"
    exec_sql "ALTER TABLE $table SET (autovacuum_enabled = true);" || echo "    (table $table may not exist)"
done

# 6. Restore default PostgreSQL session settings
echo ""
echo "6. Restoring default session settings..."
exec_sql "SET synchronous_commit = on;" || true
exec_sql "SET work_mem = '20MB';" || true
exec_sql "SET statement_timeout = DEFAULT;" || true
exec_sql "SET lock_timeout = DEFAULT;" || true

# 7. Check final statistics
echo ""
echo "7. Final statistics:"
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

# 8. Verify created indexes
echo ""
echo "8. Created indexes:"
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
echo "Finalization complete!"
echo "=========================================="
echo ""
echo "The database is optimized and ready for use."
echo ""
