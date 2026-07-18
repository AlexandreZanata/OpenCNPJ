#!/bin/bash

# Script to recreate indexes after import.
# Uses CREATE INDEX CONCURRENTLY to avoid blocking the database during creation.

set -e

echo "=========================================="
echo "Recreating indexes after import"
echo "=========================================="

# Verify the container is running
if ! docker ps | grep -q receita-postgres; then
    echo "ERROR: receita-postgres container is not running!"
    exit 1
fi

# Configure variables
DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

echo ""
echo "Connecting to PostgreSQL..."
echo "Database: $DB_NAME"
echo "User: $DB_USER"

# Execute SQL
exec_sql() {
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -c "$1"
}

# Create index concurrently if it does not exist
create_index_concurrently() {
    local index_name=$1
    local index_def=$2
    echo "  Creating index: $index_name"
    exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS $index_def;" || echo "    (index $index_name already exists or error)"
}

# 1. Recreate empresa indexes
echo ""
echo "1. Recreating empresa indexes..."
create_index_concurrently "idx_empresas_razao_social_gin" "idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops)"
create_index_concurrently "idx_empresas_natureza_juridica" "idx_empresas_natureza_juridica ON empresas(natureza_juridica)"
create_index_concurrently "idx_empresas_porte" "idx_empresas_porte ON empresas(porte_empresa)"

# 2. Recreate estabelecimento indexes
echo ""
echo "2. Recreating estabelecimento indexes..."
create_index_concurrently "idx_estab_uf_cnpj_completo" "idx_estab_uf_cnpj_completo ON estabelecimentos(cnpj_completo)"
create_index_concurrently "idx_estabelecimentos_cnpj_basico" "idx_estabelecimentos_cnpj_basico ON estabelecimentos(cnpj_basico)"
create_index_concurrently "idx_estabelecimentos_cnae" "idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal)"
create_index_concurrently "idx_estabelecimentos_municipio" "idx_estabelecimentos_municipio ON estabelecimentos(municipio)"
create_index_concurrently "idx_estabelecimentos_uf" "idx_estabelecimentos_uf ON estabelecimentos(uf)"
create_index_concurrently "idx_estabelecimentos_situacao" "idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral)"
create_index_concurrently "idx_estabelecimentos_nome_fantasia_gin" "idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops)"
create_index_concurrently "idx_estabelecimentos_cep" "idx_estabelecimentos_cep ON estabelecimentos(cep)"
create_index_concurrently "idx_estabelecimentos_cnae_uf_situacao" "idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral)"

# 3. Recreate socios indexes
echo ""
echo "3. Recreating socios indexes..."
create_index_concurrently "idx_socios_cnpj_basico" "idx_socios_cnpj_basico ON socios(cnpj_basico)"
create_index_concurrently "idx_socios_nome_socio_gin" "idx_socios_nome_socio_gin ON socios USING gin(nome_socio gin_trgm_ops)"

# 4. Recreate simples indexes
echo ""
echo "4. Recreating simples indexes..."
create_index_concurrently "idx_simples_cnpj_basico" "idx_simples_cnpj_basico ON simples(cnpj_basico)"

# 5. Verify created indexes
echo ""
echo "5. Verifying created indexes..."
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
echo "Indexes recreated successfully!"
echo "=========================================="
echo ""
