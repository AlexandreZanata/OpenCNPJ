# Comando para Importação Manual

## ✅ Status: Importação Funcionando!

A estrutura do banco foi criada corretamente e a importação está funcionando:
- ✅ Todas as tabelas de referência criadas e importadas
- ✅ Tabelas particionadas (empresas, estabelecimentos, socios) com 10 partições cada
- ✅ Índices GIN para busca fuzzy criados
- ✅ Foreign keys configuradas corretamente
- ✅ Extensões PostgreSQL (pg_trgm, btree_gin) instaladas
- ✅ Importação de tabelas de referência testada e funcionando

## ⚠️ Problemas Conhecidos e Soluções

### 1. Duplicate Key Errors
Se você já importou dados antes, pode receber erros de "duplicate key". Isso é normal e pode ser ignorado se você quiser continuar importando novos dados.

**Solução:** Limpar dados antes de reimportar (se necessário):
```bash
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "TRUNCATE empresas, estabelecimentos, socios, simples CASCADE;"
```

### 2. Foreign Key Violations
Alguns códigos nos arquivos podem não existir nas tabelas de referência. O código agora trata isso graciosamente.

## Comandos para Importação

### Importação Completa (Recomendado)
```bash
cd /home/zanata/GolandProjects/BUSCA-CNPJ-2026

# Importação completa de tudo
go run cmd/importer/main.go \
  --data-path=./data \
  --workers=16 \
  --batch-size=10000
```

### Importação por Etapas

#### 1. Tabelas de Referência (já feito ✅)
```bash
go run cmd/importer/main.go \
  --data-path=./data \
  --workers=4 \
  --batch-size=5000 \
  --skip-empresas \
  --skip-estabelecimentos \
  --skip-socios \
  --skip-simples
```

**Resultado esperado:**
- CNAEs: ~1.359 registros ✅
- Motivos: ~63 registros ✅
- Municípios: ~5.572 registros ✅
- Naturezas: ~91 registros ✅
- Países: ~255 registros ✅
- Qualificações: ~68 registros ✅

#### 2. Empresas
```bash
go run cmd/importer/main.go \
  --data-path=./data \
  --workers=8 \
  --batch-size=10000 \
  --skip-refs \
  --skip-estabelecimentos \
  --skip-socios \
  --skip-simples
```

#### 3. Estabelecimentos e Sócios (em paralelo)
```bash
go run cmd/importer/main.go \
  --data-path=./data \
  --workers=16 \
  --batch-size=10000 \
  --skip-refs \
  --skip-empresas \
  --skip-simples
```

#### 4. Simples (deve ser importado DEPOIS de empresas)
```bash
go run cmd/importer/main.go \
  --data-path=./data \
  --workers=4 \
  --batch-size=10000 \
  --skip-refs \
  --skip-empresas \
  --skip-estabelecimentos \
  --skip-socios
```

## Verificar Status da Importação

```bash
# Verificar quantos registros foram importados
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "
SELECT 
  'cnaes' as tabela, COUNT(*) as total FROM cnaes 
UNION ALL SELECT 'empresas', COUNT(*) FROM empresas 
UNION ALL SELECT 'estabelecimentos', COUNT(*) FROM estabelecimentos 
UNION ALL SELECT 'socios', COUNT(*) FROM socios 
UNION ALL SELECT 'simples', COUNT(*) FROM simples 
ORDER BY tabela;
"

# Verificar estrutura das tabelas
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "\dt"

# Verificar partições
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "
SELECT tablename FROM pg_tables 
WHERE schemaname = 'public' AND tablename LIKE '%_p%' 
ORDER BY tablename;
"
```

## Limpar Dados (se necessário)

```bash
# Limpar todas as tabelas principais (mantém tabelas de referência)
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "
TRUNCATE empresas, estabelecimentos, socios, simples CASCADE;
"

# OU limpar tudo e começar do zero
docker exec -i receita-postgres psql -U receita_user -d receita_db -c "
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
"
# Depois rode as migrations novamente
```

## Configuração

As portas dos containers estão configuradas em `config/config.yaml`:
- PostgreSQL: porta 5434
- Redis: porta 6380  
- ClickHouse: porta 9001

## Notas Importantes

- ⚠️ **Simples deve ser importado DEPOIS de empresas** (tem foreign key)
- ⚠️ Se receber erros de duplicate key, significa que os dados já foram importados antes
- ⚠️ Alguns parâmetros PostgreSQL (max_wal_size, checkpoint_timeout) já estão configurados no docker-compose.yml
- A importação pode levar várias horas dependendo do tamanho dos arquivos
- Use `--workers` baseado no número de CPUs disponíveis (recomendado: CPU * 2)
- `--batch-size` de 5000-10000 é otimizado para performance
- Se a importação falhar, você pode continuar de onde parou usando os flags `--skip-*`
