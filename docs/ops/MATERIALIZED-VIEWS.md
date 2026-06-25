# Materialized views (OpenCNPJ plan 02 Phase 4)

Analytics and lookup typeahead read from PostgreSQL materialized views refreshed concurrently.

## Views

| Materialized view | Purpose |
|-------------------|---------|
| `mv_stats_estabelecimentos_by_uf` | `/api/v1/stats/uf`, analytics summary |
| `mv_stats_estabelecimentos_by_cnae` | `/api/v1/stats/cnae` |
| `mv_stats_estabelecimentos_by_cnae_uf` | `/api/v1/stats/cnae/:cnae/uf` |
| `mv_lookup_cnaes` | `/api/v1/lookup/cnae` |
| `mv_lookup_municipios` | `/api/v1/lookup/municipio` |

Migration: `migrations/000013_materialized_views.up.sql`

## Refresh

```bash
./scripts/refresh_stats_aggregates.sh
# or: SELECT * FROM refresh_estabelecimento_stats();
```

Uses `REFRESH MATERIALIZED VIEW CONCURRENTLY` (requires unique indexes on each MV).

Run after full import and on a schedule (e.g. nightly cron on VPS):

```cron
0 3 * * * cd /opt/opencnpj && ./scripts/refresh_stats_aggregates.sh >> /var/log/opencnpj-mv-refresh.log 2>&1
```

## Verify

```bash
go run ./cmd/migrate
./scripts/refresh_stats_aggregates.sh
./scripts/opencnpj_advanced_phase4.sh http://localhost:8080
```

## Rollback

```bash
go run ./cmd/migrate down 1   # applies 000013 down — restores aggregate tables
```
