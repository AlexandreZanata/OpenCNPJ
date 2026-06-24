# High Query Routes (>10 SQL per request)

Routes listed here exceed the query budget and need review to avoid N+1 and memory leaks in production.

| Route | Method | Typical queries | Reason | Status |
|-------|--------|-----------------|--------|--------|
| _none above threshold_ | | | | |

## Under budget (documented)

| Route | Method | Typical queries | Notes |
|-------|--------|-----------------|-------|
| `/api/v1/empresas/search` | GET | 5 | 1 search + 4 enrichment (`loadRelatedByBasicos`) |
| `/api/v1/estabelecimentos/search` | GET | 5 | same pattern |
| `/api/v1/estabelecimentos/:cnpj` | GET | 5 | lookup + enrichment |

See `docs/benchmarks/2026-06-24-api-search-performance.md`.
