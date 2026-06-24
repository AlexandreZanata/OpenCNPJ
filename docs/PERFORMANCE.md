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

## Local benchmarks

Scripts live under `.local/01-api-performance-optimization/benchmarks/`.

```bash
k6 run .local/01-api-performance-optimization/benchmarks/k6-baseline.js
```
