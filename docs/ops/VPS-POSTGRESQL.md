# PostgreSQL production profile (OpenCNPJ plan 02 Phase 2)

**Example templates** for a ~16 GB RAM VPS. Complements Phase 1 (`docs/ops/VPS-OS-TUNING.md`).

> Repo ships `deploy/vps/*.example` only. Copy to the VPS, edit for your host, never commit real `*.conf` / `*.sql` under `deploy/vps/`.

## Scope

| Example template | Purpose |
|------------------|---------|
| `deploy/vps/postgresql-opencnpj.conf.example` | Memory, WAL, planner, parallelism |
| `deploy/vps/postgresql-autovacuum-opencnpj.conf.example` | Autovacuum defaults |
| `deploy/vps/analyze-search-tables.sql.example` | Partition autovacuum + `ANALYZE` |
| `scripts/vps_apply_postgresql_conf.sh` | Install examples into `conf.d` |
| `scripts/vps_analyze_search_tables.sh` | Run ANALYZE (example or local SQL) |

Local `docker-compose.yml` keeps **import-oriented** flags — do not copy to production.

## Example GUCs vs import docker-compose

| GUC | Example (~16 GB) | Import docker-compose (dev only) |
|-----|------------------|----------------------------------|
| `shared_buffers` | 4 GB | 16 GB |
| `effective_cache_size` | 12 GB | 24 GB |
| `work_mem` | 64 MB | 256 MB |
| `autovacuum` | on | off |
| `wal_level` | replica | minimal |

## Apply on VPS

```bash
sudo cp deploy/vps/postgresql-opencnpj.conf.example /etc/postgresql/18/main/conf.d/99-opencnpj.conf
sudo cp deploy/vps/postgresql-autovacuum-opencnpj.conf.example /etc/postgresql/18/main/conf.d/99-opencnpj-autovacuum.conf
# Edit both files for your RAM/version, then:
sudo systemctl reload postgresql

./scripts/vps_analyze_search_tables.sh
```

Or: `sudo ./scripts/vps_apply_postgresql_conf.sh` (installs examples — edit before reload).

Optional local SQL override (gitignored): `deploy/vps/analyze-search-tables.sql`

## Verify

```bash
./scripts/opencnpj_advanced_phase2.sh http://localhost:8080
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase2.sh http://localhost:8080
```

## References

- PostgreSQL [resource configuration](https://www.postgresql.org/docs/current/runtime-config-resource.html)
- PostgreSQL [autovacuum](https://www.postgresql.org/docs/current/routine-vacuuming.html#AUTOVACUUM)
- `deploy/vps/README.md`
