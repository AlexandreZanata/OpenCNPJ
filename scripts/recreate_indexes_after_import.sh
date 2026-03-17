#!/bin/bash

# Script para recriar índices após a importação
# Usa CREATE INDEX CONCURRENTLY para não bloquear o banco durante a criação

set -e

echo "=========================================="
echo "Recriando índices após importação"
echo "=========================================="

# Verificar se o container está rodando
if ! docker ps | grep -q receita-postgres; then
    echo "ERRO: Container receita-postgres não está rodando!"
    exit 1
fi

# Configurar variáveis
DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

echo ""
echo "Conectando ao PostgreSQL..."
echo "Database: $DB_NAME"
echo "User: $DB_USER"

# Função para executar SQL
exec_sql() {
    docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -c "$1"
}

# Função para criar índice concurrentmente se não existir
create_index_concurrently() {
    local index_name=$1
    local index_def=$2
    echo "  Criando índice: $index_name"
    exec_sql "CREATE INDEX CONCURRENTLY IF NOT EXISTS $index_def;" || echo "    (índice $index_name já existe ou erro)"
}

# 1. Recriar índices de empresas
echo ""
echo "1. Recriando índices de empresas..."
create_index_concurrently "idx_empresas_razao_social_gin" "idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops)"
create_index_concurrently "idx_empresas_natureza_juridica" "idx_empresas_natureza_juridica ON empresas(natureza_juridica)"
create_index_concurrently "idx_empresas_porte" "idx_empresas_porte ON empresas(porte_empresa)"

# 2. Recriar índices de estabelecimentos
echo ""
echo "2. Recriando índices de estabelecimentos..."
create_index_concurrently "idx_estabelecimentos_cnpj_completo" "idx_estabelecimentos_cnpj_completo ON estabelecimentos(cnpj_completo)"
create_index_concurrently "idx_estabelecimentos_cnpj_basico" "idx_estabelecimentos_cnpj_basico ON estabelecimentos(cnpj_basico)"
create_index_concurrently "idx_estabelecimentos_cnae" "idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal)"
create_index_concurrently "idx_estabelecimentos_municipio" "idx_estabelecimentos_municipio ON estabelecimentos(municipio)"
create_index_concurrently "idx_estabelecimentos_uf" "idx_estabelecimentos_uf ON estabelecimentos(uf)"
create_index_concurrently "idx_estabelecimentos_situacao" "idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral)"
create_index_concurrently "idx_estabelecimentos_nome_fantasia_gin" "idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops)"
create_index_concurrently "idx_estabelecimentos_cep" "idx_estabelecimentos_cep ON estabelecimentos(cep)"
create_index_concurrently "idx_estabelecimentos_cnae_uf_situacao" "idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral)"

# 3. Recriar índices de socios
echo ""
echo "3. Recriando índices de socios..."
create_index_concurrently "idx_socios_cnpj_basico" "idx_socios_cnpj_basico ON socios(cnpj_basico)"
create_index_concurrently "idx_socios_nome_socio_gin" "idx_socios_nome_socio_gin ON socios USING gin(nome_socio gin_trgm_ops)"

# 4. Recriar índices de simples
echo ""
echo "4. Recriando índices de simples..."
create_index_concurrently "idx_simples_cnpj_basico" "idx_simples_cnpj_basico ON simples(cnpj_basico)"

# 5. Verificar índices criados
echo ""
echo "5. Verificando índices criados..."
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
echo "Índices recriados com sucesso!"
echo "=========================================="
echo ""
