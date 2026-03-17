#!/bin/bash

# Script para dropar índices não-essenciais antes da importação
# Isso acelera significativamente a importação (3-5x mais rápido)

set -e

echo "=========================================="
echo "Dropando índices não-essenciais para importação rápida"
echo "=========================================="

# Verificar se o container está rodando
if ! docker ps | grep -q receita-postgres; then
    echo "ERRO: Container receita-postgres não está rodando!"
    echo "Execute: docker-compose up -d postgres"
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

# Função para dropar índice se existir
drop_index_if_exists() {
    local index_name=$1
    local table_name=$2
    echo "  Verificando índice: $index_name"
    exec_sql "DROP INDEX IF EXISTS $index_name CASCADE;" || echo "    (índice $index_name não existe ou erro)"
}

# 1. Dropar índices de empresas
echo ""
echo "1. Dropando índices de empresas..."
drop_index_if_exists "idx_empresas_razao_social_gin" "empresas"
drop_index_if_exists "idx_empresas_natureza" "empresas"
drop_index_if_exists "idx_empresas_natureza_juridica" "empresas"
drop_index_if_exists "idx_empresas_porte" "empresas"

# 2. Dropar índices de estabelecimentos
echo ""
echo "2. Dropando índices de estabelecimentos..."
drop_index_if_exists "idx_estabelecimentos_cnpj_completo" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_cnpj_basico" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_cnae" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_municipio" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_uf" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_situacao" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_nome_fantasia_gin" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_cep" "estabelecimentos"
drop_index_if_exists "idx_estabelecimentos_cnae_uf_situacao" "estabelecimentos"

# 3. Dropar índices de socios
echo ""
echo "3. Dropando índices de socios..."
drop_index_if_exists "idx_socios_cnpj_basico" "socios"
drop_index_if_exists "idx_socios_nome_socio_gin" "socios"

# 4. Dropar índices de simples
echo ""
echo "4. Dropando índices de simples..."
drop_index_if_exists "idx_simples_cnpj_basico" "simples"

# 5. Verificar índices restantes (apenas PRIMARY KEYs devem permanecer)
echo ""
echo "5. Índices restantes (apenas PRIMARY KEYs devem permanecer):"
exec_sql "
SELECT 
    schemaname,
    tablename,
    indexname,
    indexdef
FROM pg_indexes
WHERE schemaname = 'public'
  AND tablename IN ('empresas', 'estabelecimentos', 'socios', 'simples')
ORDER BY tablename, indexname;
"

echo ""
echo "=========================================="
echo "Índices dropados com sucesso!"
echo "=========================================="
echo ""
echo "Nota: Os índices serão recriados após a importação pelo script:"
echo "  ./scripts/recreate_indexes_after_import.sh"
echo ""
echo "Próximo passo: Execute a importação"
echo ""
