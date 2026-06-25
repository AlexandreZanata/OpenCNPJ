# VPS deployment — OpenCNPJ (examples only)

> **Repo policy:** only `*.example` templates and this README are tracked.  
> Real host configs, secrets, and tuned values live **on the VPS** (or in gitignored local copies).

## Workflow

1. Copy an example file to the target path on the VPS.
2. Edit values for your RAM, disk, and PostgreSQL version/path.
3. Apply and verify with the gate scripts (`STRICT_VPS=1` on the host).

```bash
# Phase 1 — OS (run as root on VPS)
sudo cp deploy/vps/sysctl-opencnpj.conf.example /etc/sysctl.d/99-opencnpj.conf
sudo cp deploy/vps/limits-postgres.conf.example /etc/security/limits.d/99-opencnpj-postgres.conf
sudo cp deploy/vps/99-opencnpj-io-scheduler.rules.example /etc/udev/rules.d/99-opencnpj-io-scheduler.rules
# Edit files, then:
sudo sysctl --system
sudo udevadm control --reload-rules && sudo udevadm trigger

# Phase 2 — PostgreSQL (adjust PG_CONF_D for your distro/version)
sudo ./scripts/vps_apply_postgresql_conf.sh
sudo systemctl reload postgresql
./scripts/vps_analyze_search_tables.sh
```

## Example templates (tracked)

| File | Purpose |
|------|---------|
| `sysctl-opencnpj.conf.example` | Kernel `vm.*`, `net.*`, `kernel.shm*` |
| `limits-postgres.conf.example` | `nofile` / `nproc` for `postgres` user |
| `99-opencnpj-io-scheduler.rules.example` | `mq-deadline` on SSD/NVMe |
| `fstab-postgres.example` | Mount options reference |
| `postgresql-opencnpj.conf.example` | Memory, WAL, planner GUCs |
| `postgresql-autovacuum-opencnpj.conf.example` | Autovacuum defaults |
| `analyze-search-tables.sql.example` | Partition autovacuum + `ANALYZE` |
| `meilisearch-opencnpj.env.example` | Meilisearch RAM cap + master key (Phase 5) |

## Gitignored (never commit)

Any non-`*.example` file under `deploy/vps/` — e.g. copies you keep locally while editing:

- `deploy/vps/sysctl-opencnpj.conf`
- `deploy/vps/postgresql-opencnpj.conf`
- `deploy/vps/analyze-search-tables.sql`

## Verify

```bash
./scripts/opencnpj_advanced_phase1.sh http://localhost:8080
./scripts/opencnpj_advanced_phase2.sh http://localhost:8080
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase1.sh http://localhost:8080   # on VPS after apply
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase2.sh http://localhost:8080
```

Runbooks: `docs/ops/VPS-OS-TUNING.md`, `docs/ops/VPS-POSTGRESQL.md`

## Forbidden on production VPS

Do **not** copy these from local `docker-compose.yml` import profile:

- `autovacuum=off`
- `full_page_writes=off`
- `wal_level=minimal`
- `fsync=off` / `synchronous_commit=off`
