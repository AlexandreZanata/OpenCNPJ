# Data access & API performance stack (Phase 12)

> Cross-cutting stack for SaaS API hot paths: **sqlc + pgx v5**, handwritten SQL, parallel I/O with bounded goroutines.

## Principles

| Rule | Rationale |
|------|-----------|
| **No ORM** | ORM reflection/allocation overhead hurts p99 on high-QPS lookup |
| **Handwritten SQL** | You own the query plan; tune with `EXPLAIN` |
| **sqlc CLI** | `.sql` → type-safe Go; compile-time checks, zero runtime mapping magic |
| **pgx v5** | Native Postgres driver + pool; faster than `database/sql` + `lib/pq` on hot paths |
| **Goroutines** | Parallelize **independent** I/O — never parallelize the same DB row write |
| **Indexes first** | Every new query ships with `EXPLAIN (ANALYZE, BUFFERS)` proof before merge |

## Stack layout

```
db/
  queries/
    cnpj/          # read-only — opencnpj_cnpj
      estabelecimento.sql
      empresa.sql
      socios.sql
      simples.sql
    saas/          # read/write — opencnpj_saas
      api_keys.sql
      api_usage.sql
  schema/
    cnpj.sql       # sqlc schema snapshot
sqlc.yaml          # two sqlc packages (cnpj + saas)
internal/db/
  cnpj/            # sqlc generated (DO NOT EDIT)
  saas/            # sqlc generated (DO NOT EDIT)
internal/database/
  cnpj_pgx.go      # CNPJPool (read-only)
  saas_pgx.go      # SaaSPool (read/write)
```

## sqlc workflow

```bash
# Install once (pinned in Makefile)
make sqlc-install   # go install github.com/sqlc-dev/sqlc/cmd/sqlc@v1.29.0

make sqlc-vet       # sqlc vet
make sqlc           # sqlc generate
```

- Annotate queries: `-- name: GetEstabelecimentoByCNPJ :one`
- Use `$1` placeholders only — no string concat
- CI gate fails on `sqlc` output drift (`saas_data_access_gate.sh`)

## pgx v5 pools (API)

| Pool | Database | Mode | Config key |
|------|----------|------|------------|
| `CNPJPool` | `opencnpj_cnpj` | read-only | `database_cnpj.max_open_conns` |
| `SaaSPool` | `opencnpj_saas` | read/write | `database_saas.max_open_conns` |

Pools are separate — **never share** CNPJ and SaaS connections.

Wiring:

- `internal/cnpj/wire.go` → `cnpjdb.New(database.CNPJPool)`
- `internal/saas/wire.go` → `saasdb.New(database.SaaSPool)`

## Incremental migration off `lib/pq`

| Area | Status | Notes |
|------|--------|-------|
| Public CNPJ lookup (`GET /api/v1/cnpj/:cnpj`) | **pgx + sqlc** | Phase 4 complete |
| API key auth + usage flush | **pgx + sqlc** | Phase 3 complete |
| Admin auth (`internal/adminauth`) | **pgx + sqlc** | SaaS pool |
| `/readyz` dual-DB ping | `database/sql` + `lib/pq` | legacy; migrate when admin path moves |
| Import pipeline (`cmd/importer`) | `database/sql` + `lib/pq` | local PC only — out of VPS hot path |
| Search/export (`internal/repository`) | `database/sql` + `lib/pq` | enterprise portal; not SaaS v1 route |

**Plan:** keep `lib/pq` on import and legacy search until SaaS v1 is stable. Next target: dual `/readyz` probe via pgx ping helpers.

## Goroutine patterns (CNPJ lookup)

`GET /api/v1/cnpj/:cnpj` — after cache miss, fetch related rows **in parallel** with `errgroup` (`internal/cnpj/service.go`):

| Goroutine | Query |
|-----------|-------|
| 1 | `GetEstabelecimentoByCNPJ` |
| 2 | `GetEmpresaByBasico` |
| 3 | `ListSociosByBasico` |
| 4 | `GetSimplesByBasico` |

**Budget: ≤ 4 fan-out per lookup** (enforced in `saas_data_access_gate.sh`).

**Also async (non-blocking):**

| Work | Pattern |
|------|---------|
| Redis usage `INCR` | `UsageTracker` background flush |
| Rate limit | Redis middleware before handler |
| L1/Redis cache populate | `services.GetOrSetJSON` |

**Never:**

- Spawn unbounded goroutines per request
- Parallel writes to the same usage row without `ON CONFLICT`
- Replace indexes/partitions with “more goroutines”

## Index checklist

### CNPJ DB (`opencnpj_cnpj`)

| Query | Required index | Verify |
|-------|----------------|--------|
| `WHERE cnpj_completo = $1` (schema / local) | `idx_estabelecimentos_cnpj_completo` | `Index Scan` / `Bitmap Index Scan` |
| `WHERE cnpj_completo = $1` (**VPS UF partitions**) | `idx_estab_uf_cnpj_completo` via `scripts/vps_create_indexes.sql` | never seq-scan all UFs |
| `WHERE cnpj_basico = $1` (empresa) | PK on `empresas` | `Index Scan` |
| socios by `cnpj_basico` | `idx_socios_cnpj_basico` | no seq scan |
| simples by `cnpj_basico` | PK on `simples` | index only |

> **VPS gotcha:** PostgreSQL index names are schema-global. If a leftover
> `idx_estabelecimentos_cnpj_completo` exists on `estabelecimentos_legacy_range`,
> `CREATE INDEX IF NOT EXISTS` on the UF parent is a no-op and lookups fall back to
> multi-second parallel seq scans. Always use the `idx_estab_uf_*` names on VPS.

```sql
EXPLAIN (ANALYZE, BUFFERS)
SELECT e.cnpj_completo FROM estabelecimentos e WHERE e.cnpj_completo = '00000000000191';
```

### SaaS DB (`opencnpj_saas`)

| Table | Index | Purpose |
|-------|-------|---------|
| `api_keys` | `idx_api_keys_hash` — `(key_hash) WHERE revoked_at IS NULL` | O(1) auth lookup |
| `api_keys` | `(client_id) WHERE revoked_at IS NULL` | admin list keys |
| `api_clients` | `(status) WHERE status = 'active'` | filter suspended |
| `api_usage_daily` | PK `(client_id, date)` | upsert flush |

Migrations: `000001_saas_metadata`, `000002_saas_indexes`, `000003_api_key_index_rename`.

## Performance targets

| Gate | Target |
|------|--------|
| CNPJ lookup p95 (warm L1+Redis) | < 50 ms |
| CNPJ lookup p95 (cache miss, VPS) | < 150 ms |
| API key middleware overhead | < 5 ms p95 |
| Goroutine fan-out | ≤ 4 per lookup request |
| `sqlc vet` + `go test` | green in CI |

## Gate (CI / local)

```bash
./scripts/saas_data_access_gate.sh           # sqlc + unit tests
./scripts/saas_data_access_gate.sh --docker  # + EXPLAIN integration
go test ./internal/perfvalidation/ -run TestPhase12 -count=1
```

Specialized gates (also valid):

```bash
./scripts/saas_api_key_gate.sh --docker      # SaaS auth EXPLAIN
./scripts/saas_public_cnpj_gate.sh --docker  # CNPJ lookup EXPLAIN
```

## Related docs

- [SAAS-VPS-DEPLOY.md](SAAS-VPS-DEPLOY.md) — SaaS overview
- [PERFORMANCE.md](../PERFORMANCE.md) — latency targets
- [MONTHLY-CNPJ-SYNC.md](MONTHLY-CNPJ-SYNC.md) — re-run `EXPLAIN` after monthly restore (stats drift)
