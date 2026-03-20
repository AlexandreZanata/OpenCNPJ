# Performance

## PostgreSQL (bulk import)

- `synchronous_commit=off`
- `work_mem=256MB`
- `maintenance_work_mem=1GB`
- Desabilitar triggers/FKs durante COPY.
- Dropar indices nao essenciais antes da carga e recriar depois.

## Go runtime

- `GOMAXPROCS` = numero de vCPUs.
- `GOGC=200` em cargas de importacao longas.
- Buffer de leitura CSV em 4 MB.

## Metas

- Parsing puro: >= 500.000 linhas/s.
- COPY local: >= 150.000 linhas/s.
- Carga completa (~50M): <= 20 minutos em 8 vCPU / 32 GB / NVMe.

## Checklist pre-importacao

- Indices secundarios dropados.
- Triggers desabilitados nas tabelas alvo.
- Sessao de importacao com autovacuum desabilitado.
- Espaco em disco e WAL monitorados.
