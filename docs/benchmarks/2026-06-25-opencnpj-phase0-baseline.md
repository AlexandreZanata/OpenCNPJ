# OpenCNPJ Advanced — Phase 0 Baseline

- **Date**: 2026-06-25
- **Environment**: local Docker + API on host (post plan 01)
- **Dataset**: local import (not full 150M VPS row count)

## Phase 0 gate

```bash
./scripts/opencnpj_advanced_phase0.sh http://localhost:8080
# 9/9 passed
```

## k6 (`k6-full.js`, warm cache, 10 VU × 30s)

| Metric | p99 | Threshold | Result |
|--------|-----|-----------|--------|
| `cnpj_lookup_ms` | 1.88 ms | < 100 ms | pass |
| `search_ms` | 2.52 ms | < 500 ms | pass |
| `uf_search_ms` | 3.97 ms | < 500 ms | pass |
| `http_req_failed` | 0% | < 1% | pass |

Artifacts: `.local/02-opencnpj-advanced-optimization/benchmarks/k6-advanced-baseline-20260625.json`

## Plan 01 prerequisites confirmed

- pgBouncer container active
- Partial indexes `000011`, FTS `000012`
- Cache metrics `busca_cnpj_cache_hits_total` / `busca_cnpj_cache_misses_total`
- Keyset pagination + gzip (via `api_perf_validation.sh`)

## Notes

- Partition pruning EXPLAIN templates saved; current schema uses HASH(`cnpj_basico`) — UF LIST migration is Phase 6.
- VPS 150M-row baseline should re-run `./scripts/opencnpj_advanced_baseline.sh` on staging.
