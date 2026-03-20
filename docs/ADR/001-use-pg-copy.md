# ADR 001 - Use PostgreSQL COPY for ingest

## Status
Accepted

## Context
O volume de dados da RFB exige throughput alto e baixo overhead por linha.

## Decision
Usar `pgx.CopyFrom` como estrategia padrao de ingestao, mantendo batch insert apenas como fallback.

## Consequences
- Melhor throughput em comparacao a INSERT linha a linha.
- Fluxo de importacao fica dependente de schema estavel por lote.
