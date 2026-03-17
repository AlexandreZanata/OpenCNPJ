# Comandos para Limpar Banco e Reimportar Dados

## ⚠️ IMPORTANTE: PostgreSQL 18 com UUID v7

Este projeto agora usa **PostgreSQL 18** com **UUID v7 nativo** para máxima performance.

**Nota:** PostgreSQL 18 usa a função `uuidv7()` (não `uuid_generate_v7()`).

## 🚀 Comandos Completos para Reimportação

### ⚠️ IMPORTANTE: Recriar PostgreSQL 18

Se você já tinha PostgreSQL 15 rodando, precisa **recriar o container** para usar PostgreSQL 18:

**Opção 1: Usar script automático**
```bash
cd /home/zanata/GolandProjects/BUSCA-CNPJ-2026
./recriar_postgres_18.sh
```

**Opção 2: Comandos manuais**
```bash
cd /home/zanata/GolandProjects/BUSCA-CNPJ-2026

# Parar PostgreSQL
docker compose down postgres

# Remover volume (força recriação com PostgreSQL 18)
docker volume rm busca-cnpj-2026_postgres_data

# Subir PostgreSQL 18
docker compose up -d postgres

# Aguardar estar pronto
sleep 10

# Verificar versão
docker compose exec -T postgres psql -U receita_user -d receita_db -c "SELECT version();"
```

**Deve mostrar:** `PostgreSQL 18.x` na saída

### 1. Limpar Banco de Dados (se necessário)

```bash
docker exec -i receita-postgres psql -U receita_user -d receita_db <<EOF
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
GRANT ALL ON SCHEMA public TO receita_user;
GRANT ALL ON SCHEMA public TO public;
EOF
```

### 3. Limpar Banco de Dados Completamente

```bash
docker exec -i receita-postgres psql -U receita_user -d receita_db <<EOF
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
GRANT ALL ON SCHEMA public TO receita_user;
GRANT ALL ON SCHEMA public TO public;
EOF
```

### 4. Rodar Migrations (incluindo UUID v7)

```bash
# As migrations serão executadas automaticamente ao iniciar a aplicação
# Ou execute manualmente:
go run cmd/api/main.go
# (Isso executará as migrations automaticamente)
```

**Ou execute migrations manualmente:**

```bash
# Instalar migrate CLI se necessário:
# go install -tags 'postgres' github.com/golang-migrate/migrate/v4/cmd/migrate@latest

# Executar migrations:
migrate -path ./migrations -database "postgres://receita_user:receita_password@localhost:5434/receita_db?sslmode=disable" up
```

### 5. Reimportar Todos os Dados (Otimizado PostgreSQL 18)

```bash
cd /home/zanata/GolandProjects/BUSCA-CNPJ-2026

# Importação completa com configurações otimizadas
# O banco será limpo automaticamente antes da importação
go run cmd/importer/main.go \
  --data-path=./data \
  --workers=32 \
  --batch-size=25000
```

**Configurações Otimizadas:**
- `--workers=32`: Workers = CPU cores × 4 (ajuste conforme seu hardware)
- `--batch-size=25000`: Batch size otimizado para PostgreSQL 18 (20k-50k é ideal)

**Limpeza Automática:**
- Por padrão, o comando **sempre limpa o banco** antes de importar
- Todas as tabelas são truncadas (empresas, estabelecimentos, socios, simples, e tabelas de referência)
- Use `--no-clean` se quiser manter dados existentes: `go run cmd/importer/main.go --no-clean ...`

### 6. Verificar Importação

```bash
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "
SELECT 
  'cnaes' as tabela, COUNT(*) as total FROM cnaes 
UNION ALL SELECT 'empresas', COUNT(*) FROM empresas 
UNION ALL SELECT 'estabelecimentos', COUNT(*) FROM estabelecimentos 
UNION ALL SELECT 'socios', COUNT(*) FROM socios 
UNION ALL SELECT 'simples', COUNT(*) FROM simples 
ORDER BY tabela;
"

# Verificar UUIDs foram gerados
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "
SELECT 
  'empresas' as tabela, COUNT(*) as total, COUNT(id) as com_uuid FROM empresas
UNION ALL 
SELECT 'estabelecimentos', COUNT(*), COUNT(uuid_id) FROM estabelecimentos
UNION ALL 
SELECT 'socios', COUNT(*), COUNT(uuid_id) FROM socios
UNION ALL 
SELECT 'simples', COUNT(*), COUNT(uuid_id) FROM simples;
"
```

### 7. Criar Índices CONCURRENTLY (se necessário)

```bash
# Os índices já são criados nas migrations, mas se precisar recriar:
docker exec -i receita-postgres psql -U receita_user -d receita_db <<EOF
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_empresas_uuid ON empresas(id);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_estabelecimentos_uuid ON estabelecimentos(uuid_id);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_socios_uuid ON socios(uuid_id);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_simples_uuid ON simples(uuid_id);
EOF
```

### 8. VACUUM ANALYZE Final

```bash
docker exec -i receita-postgres psql -U receita_user -d receita_db <<EOF
VACUUM ANALYZE empresas;
VACUUM ANALYZE estabelecimentos;
VACUUM ANALYZE socios;
VACUUM ANALYZE simples;
VACUUM ANALYZE cnaes;
EOF
```

## 📊 Performance Esperada

### Importação
- **Antes (PostgreSQL 15):** ~4-6 horas para 150 milhões de registros
- **Depois (PostgreSQL 18 + otimizações):** ~2-3 horas (50% mais rápido)

### Exportação
- **Antes:** SELECT + CSV writer → ~30s para 100k registros, problemas de memória acima de 1M
- **Depois:** Função PostgreSQL + streaming → ~5-10s para 100k registros, suporta milhões sem problemas

## 🔧 Configurações PostgreSQL 18 Otimizadas

As seguintes configurações já estão no `docker-compose.yml`:

- `maintenance_work_mem = 4GB` (aumentado de 2GB)
- `max_parallel_workers_per_gather = 8` (aumentado de 4)
- `max_parallel_maintenance_workers = 8` (novo)
- `parallel_setup_cost = 0` (forçar paralelismo)
- `parallel_tuple_cost = 0.001` (custo baixo para paralelismo)
- `enable_partitionwise_join = on` (otimizar joins em tabelas particionadas)
- `enable_partitionwise_aggregate = on` (agregações mais rápidas)

## 📝 Notas Importantes

1. **UUID v7:** Todos os registros terão UUID v7 gerado automaticamente via `uuidv7()` nativo do PostgreSQL 18
2. **Chaves Primárias:** Mantidas baseadas em CNPJ para compatibilidade
3. **UNLOGGED Tables:** Usadas durante importação para melhor performance (2-3x mais rápido)
4. **Batch Size:** Aumentado para 25.000 (otimizado para PostgreSQL 18)
5. **Workers:** Aumentado para CPU cores × 4 para melhor paralelismo

## 🐛 Troubleshooting

### Erro: "function uuidv7() does not exist" ou "function uuid_generate_v7() does not exist"
- **Causa:** Container PostgreSQL ainda está na versão antiga (15 ou anterior)
- **Solução:** 
  1. Execute `./recriar_postgres_18.sh` para recriar o container com PostgreSQL 18
  2. Ou manualmente: `docker compose down postgres && docker volume rm busca-cnpj-2026_postgres_data && docker compose up -d postgres`
  3. Verifique a versão: `docker compose exec -T postgres psql -U receita_user -d receita_db -c "SELECT version();"`

### Erro: "duplicate key value violates unique constraint"
- **Causa:** Dados já foram importados
- **Solução:** Limpe o banco primeiro (passo 3)

### Importação lenta
- **Verifique:** Número de workers e batch size
- **Ajuste:** Workers = CPU cores × 4, Batch size = 20k-50k

### Problemas de memória durante exportação
- **Causa:** Método antigo (SELECT + CSV writer)
- **Solução:** Use o novo método ExportToCSV que faz streaming direto

## ✅ Checklist Final

- [ ] PostgreSQL 18 está rodando
- [ ] Banco foi limpo completamente
- [ ] Migrations foram executadas (incluindo UUID v7)
- [ ] Dados foram importados com sucesso
- [ ] UUIDs foram gerados para todos os registros
- [ ] VACUUM ANALYZE foi executado
- [ ] Performance está dentro do esperado
