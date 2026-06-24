# API Search Performance — P0–P2 Validation

- **Date**: 2026-06-24
- **Environment**: local Docker (`postgres:18.4`, pgBouncer, Redis, Go API)
- **Dataset**: post-import CNPJ sample (production-like volume)
- **Scope**: Phases 0–7 from `.local/01-api-performance-optimization/`

## Summary

| Optimization tier | Status | Key outcome |
|-------------------|--------|-------------|
| P0 — PostgreSQL + pgBouncer | Applied | Trigram search uses GIN indexes; pool via port 6432 |
| P1 — Redis / HTTP / sonic | Applied | Per-route TTL, msgpack cache, gzip (~76% payload reduction) |
| P2 — Keyset + FTS | Applied | `next_cursor` pagination; multi-word `tsvector` search |

## Latency samples (curl, seconds)

| Endpoint | Cold / miss | Warm / cached |
|----------|-------------|---------------|
| `GET /estabelecimentos/33000167000101` | 7.46 | 0.0006 |
| `GET /empresas/search?razao_social=PETROBRAS` | 4.43–5.00 | 0.001 |
| `GET /empresas/search?razao_social=PETRO+BRAS` (FTS) | 1.72 | — |
| Keyset page 2 (`cursor=`) | — | 0.001 |
| Offset page 25 (`offset=480`) | 0.24 | — |

**Notes:**

- First search request may hit the 5s Fiber timeout under heavy DB load; warm cache is sub-millisecond.
- Keyset page 2 avoids OFFSET scan cost; deep offset (480) still faster than uncached first page but slower than cursor+cache.

## Success criteria (from optimization plan)

| Metric | Target | Measured |
|--------|--------|----------|
| CNPJ lookup p95 (cached) | < 50 ms | ~0.6 ms |
| CNPJ lookup p95 (miss) | < 200 ms | ~7.5 s (enrichment on cold cache — needs cache warming) |
| Search p95 | < 500 ms | ~5 s cold; < 2 ms warm |
| Redis hit rate (steady) | > 40% | 60% (9 hits / 15 ops) — low sample |
| SQL queries per search | ≤ 2 budget goal | **5** (1 search + 4 enrichment) — under 10-route threshold |

## PostgreSQL (`pg_stat_statements`)

Top runtime queries after P2 (excluding DDL):

| Avg ms | Calls | Query pattern |
|--------|-------|---------------|
| 17.21 | 8 | Enrichment `ListEmpresasFullByBasicos` |
| 1338.21 | 7 | Estabelecimento join search |
| 670.82 | 4 | Empresa trigram search |
| 248.75 | 1 | Empresa FTS search |

FTS GIN indexes: `idx_empresas_busca_fts`, `idx_estabelecimentos_busca_fts` — present.

## Redis

```
keyspace_hits: 9
keyspace_misses: 6
```

Hit rate ~60% on low-volume manual testing. Re-run k6 for production-like steady-state.

## Validation commands

```bash
# Automated gate (Phase 8.1)
./scripts/api_perf_validation.sh http://localhost:8080

# k6 baseline
k6 run .local/01-api-performance-optimization/benchmarks/k6-baseline.js

# k6 keyset vs deep offset
k6 run .local/01-api-performance-optimization/benchmarks/k6-keyset-deep.js
```

## Follow-ups (DVT)

| ID | Item | Status |
|----|------|--------|
| DVT-15 | Keyset e2e + UI cursor nav | open |
| DVT-16 | FTS quality integration tests | open |
| DVT-17 | Meilisearch indexer + delegation | open |
| DVT-18 | CI perf gate (`api_perf_validation.sh`) | open |

## Query budget

Search routes execute **5 SQL queries** per request (1 main + 4 enrichment). Below the 10-query `HIGH_QUERY_ROUTES` threshold — no entry required.
