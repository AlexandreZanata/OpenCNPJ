# CNAE HASH sub-partitioning (OpenCNPJ plan 02 Phase 7)

Adds **HASH(`cnae_fiscal_principal`)** sub-partitions under each **LIST(`uf`)** branch on `estabelecimentos` for CNAE+UF filtered search pruning.

## Scope

| Item | Detail |
|------|--------|
| Migration | `migrations/000016_cnae_hash_subpartitions.up.sql` |
| Top level | LIST(`uf`) — 28 estado codes + `EX` + `DEFAULT` |
| Sub level | HASH(`cnae_part`) — 4 buckets per UF branch (`cnae_part` = COALESCE(cnae, `0000000`)) |
| Leaf count | 29 × 4 = **116** hash leaves (+ 29 UF intermediates) |
| PK | `(uuid_id, cnpj_basico, uf, cnae_part)` — maintained by `trg_estabelecimentos_cnae_part` |
| Stats MVs | Dropped/recreated; refreshed at end of migration |

## Apply (off-peak)

Requires Phase 6 (`000014`) already applied.

```bash
go run ./cmd/migrate
./scripts/explain_cnae_uf_partition_pruning.sh
STRICT=1 ./scripts/opencnpj_advanced_phase7.sh http://localhost:8080
```

**Downtime:** full table copy (`INSERT … SELECT`) — plan maintenance window on VPS (~150M rows).

## Verify pruning

```sql
EXPLAIN SELECT id FROM estabelecimentos
WHERE uf = 'SP' AND cnae_fiscal_principal = '4781400'
  AND situacao_cadastral = '02'
LIMIT 100;
-- Expect: Scan on estabelecimentos_sp_hN (single hash leaf)
```

Research gate query (before/after benchmarks on production-size copy):

```bash
mkdir -p .local/03-saas-vps-comerc-api/benchmarks
# Before 000016 (Phase 6 layout):
psql -f scripts/explain_cnae_uf_partition_pruning.sql \
  > .local/03-saas-vps-comerc-api/benchmarks/explain-cnae-uf-before.txt
# After 000016:
psql -f scripts/explain_cnae_uf_partition_pruning.sql \
  > .local/03-saas-vps-comerc-api/benchmarks/explain-cnae-uf-after.txt
```

Target after migration: `Partitions pruned: 2` (UF branch + hash leaf) when filtering on `cnae_part`; UF-only prune when filtering on `cnae_fiscal_principal` alone.

### Benchmark snapshot (71M rows, local docker)

| Query filter | Plan | Execution time |
|--------------|------|----------------|
| Before (`000014`) | Parallel Seq Scan on `estabelecimentos_sp` | ~43 ms |
| After (`000016`), `cnae_fiscal_principal` | Index Scan on `estabelecimentos_sp_h*` (4 buckets) | ~1.5 ms |
| After (`000016`), `cnae_part` | Seq/Index Scan on **one** leaf (`estabelecimentos_sp_h2`) | sub-ms planning |

For category browse (v2), add `cnae_part = $cnae` alongside `cnae_fiscal_principal` in repository queries to enable single-leaf hash pruning.

## Rollback

```bash
psql -f migrations/000016_cnae_hash_subpartitions.down.sql
```

Restores LIST(`uf`) only (Phase 6 layout).

## Import note

`COPY` / importer still targets parent `estabelecimentos`; PostgreSQL routes rows to UF branch then CNAE hash leaf automatically.

## v1 launch

CNAE partitioning is **not required for v1** (CNPJ lookup only). Schedule after Phase 4 go-live, before enabling category browse API (v2).

## References

- PostgreSQL [partition pruning](https://www.postgresql.org/docs/current/ddl-partitioning.html#DDL-PARTITION-PRUNING)
- `internal/partition/cnae_hash.go` — hash bucket constants
- `internal/partition/br_uf.go` — UF code list
- DVT-32
