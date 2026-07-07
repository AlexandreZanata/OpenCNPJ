# API Performance

Operational notes for profiling and validating API performance improvements.

## pprof (CPU / heap)

Enable the profiling server:

```bash
ENABLE_PPROF=true go run ./cmd/api
```

Default listen address: `:6060`.

```bash
# CPU profile (30s)
go tool pprof http://localhost:6060/debug/pprof/profile?seconds=30

# Heap allocations
go tool pprof http://localhost:6060/debug/pprof/heap

# Goroutines
go tool pprof http://localhost:6060/debug/pprof/goroutine
```

## Response compression

Search endpoints return gzip when the client sends `Accept-Encoding: gzip`:

```bash
curl -s -H 'Accept-Encoding: gzip' -D - \
  'http://localhost:8080/api/v1/empresas/search?razao_social=PETROBRAS&limit=5' -o /dev/null
```

Expect `Content-Encoding: gzip` on large JSON payloads.

## Cache TTL map

| Key prefix | Default TTL | Config override |
|------------|-------------|-----------------|
| `estabelecimento:cnpj` | 24h | `cache.ttl_cnpj` |
| `empresas:search` / `estabelecimentos:search` | 5m | `cache.ttl_search` |
| `stats:` | 1h | `cache.ttl_analytics` |
| `lookup:` | 15m | `cache.ttl_lookup` |

Cache values are stored as msgpack (legacy JSON keys remain readable).

## L1 cache (Ristretto, plan 02 Phase 3)

In-process L1 sits above Redis L2: **L1 → Redis → PostgreSQL**.

| Setting | Default | Notes |
|---------|---------|-------|
| `cache.l1_enabled` | `true` | Disable for minimal memory footprint |
| `cache.l1_max_cost_mb` | `256` | ~256 MB on 16 GB VPS |
| `cache.l1_num_counters` | `10000000` | Ristretto frequency sketch |
| `cache.l1_buffer_items` | `64` | Set buffer |

Prometheus: `busca_cnpj_l1_cache_hits_total`, `busca_cnpj_l1_cache_misses_total` (L2: `busca_cnpj_cache_hits_total`).

```bash
./scripts/opencnpj_advanced_phase3.sh http://localhost:8080
```

Package: `internal/cache/l1/`

## Materialized views (plan 02 Phase 4)

Analytics and lookup read from PostgreSQL MVs (`migrations/000013_*`).

```bash
go run ./cmd/migrate
./scripts/refresh_stats_aggregates.sh
./scripts/opencnpj_advanced_phase4.sh http://localhost:8080
```

Runbook: `docs/ops/MATERIALIZED-VIEWS.md`

## Keyset pagination

Search endpoints accept optional `cursor` (cannot combine with `offset`):

```bash
# First page
curl 'http://localhost:8080/api/v1/empresas/search?razao_social=PETROBRAS&limit=5'

# Next page (use next_cursor from prior response)
curl 'http://localhost:8080/api/v1/empresas/search?razao_social=PETROBRAS&limit=5&cursor=score:0.45000000|cnpj:12345678'
```

Response fields: `has_more`, `next_cursor` (omitted on last page). `offset` remains supported but deprecated.

## Full-text search (multi-word)

Queries with spaces use PostgreSQL `tsvector` + `portuguese` config instead of `pg_trgm`:

```bash
curl 'http://localhost:8080/api/v1/empresas/search?razao_social=PETRO%20BRAS&limit=5'
```

Single-word queries continue to use trigram similarity.

## Local benchmarks

Scripts live under `.local/01-api-performance-optimization/benchmarks/`.

Run all gates (Docker k6, warm cache):

```bash
BENCHMARK_MODE=true go run ./cmd/api   # disables rate limiter for load tests
./scripts/run_k6_benchmarks.sh
```

Single script:

```bash
docker run --rm --add-host=host.docker.internal:host-gateway \
  -e API_BASE_URL=http://host.docker.internal:8080 \
  -v "$(pwd)/.local/01-api-performance-optimization/benchmarks:/scripts:ro" \
  grafana/k6 run /scripts/k6-baseline.js
```

## Prefork (`server.prefork`)

Default: `false`. DB/Redis/ClickHouse init runs once in `main()` before `fiber.New`; enabling prefork forks workers that inherit `sql.DB` handles — **keep prefork disabled** when using pgBouncer (validated 2026-06-24 with k6 50 VU, p95 &lt; 1 ms on cached CNPJ lookup).

## Meilisearch (optional)

Docker service on port `7700`. Set `meilisearch.enabled: true` in `config/config.yaml` to delegate text-only search to Meili (Postgres remains source of truth for enrichment). With `selective_active_matriz: true` (default), the indexer scopes to active headquarters rows only.

```bash
go run ./cmd/meilisearch-index   # full selective re-index
go run ./cmd/meilisearch-index -max-batches 2   # dev sample
# importer auto-syncs when meilisearch.enabled is true
```

## Post-implementation validation (Phase 8)

Run after P0–P2 changes:

```bash
./scripts/api_perf_validation.sh http://localhost:8080
go test ./... -short && go vet ./...
```

Report: `docs/benchmarks/2026-06-24-api-search-performance.md`

Redis hit-rate helper: `internal/perfvalidation` (40% gate for steady load).

## OpenCNPJ advanced plan — Phase 0 gate

Before plan `02` optimizations (Ristretto L1, MVs, Meilisearch selective index):

```bash
./scripts/opencnpj_advanced_phase0.sh http://localhost:8080
./scripts/opencnpj_advanced_baseline.sh http://localhost:8080   # k6 + system snapshot
```

Artifacts: `.local/02-opencnpj-advanced-optimization/benchmarks/` (gitignored).
Report template: `docs/benchmarks/2026-06-25-opencnpj-phase0-baseline.md`

## OpenCNPJ advanced plan — Phase 1 gate (VPS OS tuning)

Kernel, ulimits, and I/O scheduler templates for 16 GB production VPS:

```bash
./scripts/opencnpj_advanced_phase1.sh http://localhost:8080
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase1.sh http://localhost:8080   # after host apply
```

Artifacts: `deploy/vps/*.example` · Runbook: `docs/ops/VPS-OS-TUNING.md`

## OpenCNPJ advanced plan — Phase 2 gate (PostgreSQL 16 GB profile)

Production `postgresql.conf` snippets for 16 GB VPS (~4 GB `shared_buffers`, 64 MB `work_mem`, autovacuum on):

```bash
./scripts/opencnpj_advanced_phase2.sh http://localhost:8080
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase2.sh http://localhost:8080   # after PG apply
./scripts/vps_analyze_search_tables.sh   # refresh planner stats
```

Artifacts: `deploy/vps/*.example` · Runbook: `docs/ops/VPS-POSTGRESQL.md`

## OpenCNPJ advanced plan — Phase 3 gate (Ristretto L1)

```bash
./scripts/opencnpj_advanced_phase3.sh http://localhost:8080
```

Requires API rebuilt with L1 enabled (`cache.l1_enabled: true`). Warm CNPJ path should show `busca_cnpj_l1_cache_hits_total` in `/metrics`.

## OpenCNPJ advanced plan — Phase 4 gate (materialized views)

```bash
./scripts/opencnpj_advanced_phase4.sh http://localhost:8080
```

Requires migration `000013` + `refresh_estabelecimento_stats()` after import.

## OpenCNPJ advanced plan — Phase 5 gate (Meilisearch selective index)

Indexes **active matriz** only (`situacao_cadastral = 02`, `identificador_matriz_filial = 1`) — ~20M docs target vs full branch set.

| Setting | Default | Notes |
|---------|---------|-------|
| `meilisearch.enabled` | `false` | Enable after Meilisearch is up |
| `meilisearch.selective_active_matriz` | `true` | Plan 02 selective scope |

```bash
docker compose up -d meilisearch
./scripts/opencnpj_advanced_phase5.sh http://localhost:8080
MEILI_STRICT=1 ./scripts/opencnpj_advanced_phase5.sh http://localhost:8080   # sample index
./scripts/meilisearch_selective_index.sh
```

Runbook: `docs/ops/MEILISEARCH-SELECTIVE.md` · Package: `internal/meilisearch/selective.go`

## OpenCNPJ advanced plan — Phase 6 gate (UF LIST partitions)

Migrates `estabelecimentos` from HASH(`cnpj_basico`) to LIST(`uf`) for UF query pruning.

```bash
go run ./cmd/migrate   # applies 000014 (off-peak on VPS)
./scripts/explain_uf_partition_pruning.sh
./scripts/opencnpj_advanced_phase6.sh http://localhost:8080
STRICT=1 ./scripts/opencnpj_advanced_phase6.sh http://localhost:8080
```

Runbook: `docs/ops/UF-PARTITIONING.md` · UF codes: `internal/partition/br_uf.go`

## OpenCNPJ advanced plan — Phase 7 gate (CNAE HASH sub-partitions)

Adds HASH(`cnae_fiscal_principal`) sub-partitions under each LIST(`uf`) branch for CNAE+UF query pruning.

```bash
go run ./cmd/migrate   # applies 000016 (off-peak on VPS; requires 000014)
./scripts/explain_cnae_uf_partition_pruning.sh
./scripts/opencnpj_advanced_phase7.sh http://localhost:8080
STRICT=1 ./scripts/opencnpj_advanced_phase7.sh http://localhost:8080
```

Runbook: `docs/ops/CNAE-PARTITIONING.md` · Buckets: `internal/partition/cnae_hash.go`
