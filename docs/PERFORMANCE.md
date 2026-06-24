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

Docker service on port `7700`. Set `meilisearch.enabled: true` in `config/config.yaml` to delegate text-only search to Meili (Postgres remains source of truth for enrichment).

```bash
go run ./cmd/meilisearch-index   # full re-index
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
