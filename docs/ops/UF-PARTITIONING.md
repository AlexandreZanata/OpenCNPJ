# UF LIST partitioning (OpenCNPJ plan 02 Phase 6)

Migrates `estabelecimentos` from **HASH(`cnpj_basico`)** (import-optimized) to **LIST(`uf`)** for UF-filtered search partition pruning.

## Scope

| Item | Detail |
|------|--------|
| Migration | `migrations/000014_uf_list_partitions.up.sql` |
| Partitions | 28 estado codes + `EX` (exterior) + `DEFAULT` |
| PK | `(uuid_id, cnpj_basico, uf)` — PostgreSQL requires partition key in PK |
| Stats MVs | Dropped/recreated; `refresh_estabelecimento_stats()` runs at end |

## Apply (off-peak)

```bash
go run ./cmd/migrate
./scripts/explain_uf_partition_pruning.sh
STRICT=1 ./scripts/opencnpj_advanced_phase6.sh http://localhost:8080
```

**Downtime:** full table copy (`INSERT … SELECT`) — plan for maintenance window on VPS (~150M rows).

## Verify pruning

```sql
EXPLAIN SELECT id FROM estabelecimentos
WHERE uf = 'SP' AND situacao_cadastral = '02' LIMIT 5;
-- Expect: Scan on estabelecimentos_sp only
```

## Rollback

```bash
# apply down migration manually or via migrate tool rollback
psql -f migrations/000014_uf_list_partitions.down.sql
```

Restores HASH(8) on `cnpj_basico`.

## Import note

`COPY` / importer still targets parent `estabelecimentos`; PostgreSQL routes rows to UF partitions automatically. Ensure `uf` is populated (required NOT NULL after migration).

## References

- PostgreSQL [partition pruning](https://www.postgresql.org/docs/current/ddl-partitioning.html#DDL-PARTITION-PRUNING)
- `internal/partition/br_uf.go` — UF code list
- DVT-25
