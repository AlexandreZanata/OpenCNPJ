# VPS OS tuning — OpenCNPJ production (plan 02 Phase 1)

Target: **Hostinger VPS 16 GB RAM**, PostgreSQL 18 + Redis + Go API + optional Meilisearch.

## Quick apply (root on VPS)

```bash
sudo cp deploy/vps/sysctl-opencnpj.conf /etc/sysctl.d/99-opencnpj.conf
sudo sysctl --system

sudo cp deploy/vps/limits-postgres.conf /etc/security/limits.d/99-opencnpj-postgres.conf

sudo cp deploy/vps/99-opencnpj-io-scheduler.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger

# Postgres data mount (example — adjust device/path)
# Add noatime,nodiratime to /etc/fstab for the volume hosting PGDATA.
```

Reboot or restart PostgreSQL after limits change.

## Verify

```bash
./scripts/opencnpj_advanced_phase1.sh http://localhost:8080
# On VPS with configs applied:
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase1.sh http://localhost:8080
```

## Forbidden on production

Do **not** apply these from local `docker-compose.yml` import profile:

- `autovacuum=off`
- `full_page_writes=off`
- `wal_level=minimal`
- `fsync=off` / `synchronous_commit=off`

## Optional: huge pages

PostgreSQL docs recommend measuring before enabling. Example (2 GB huge pages):

```bash
# /etc/sysctl.d/99-opencnpj-hugepages.conf (optional — measure first)
# vm.nr_hugepages = 1024
```

See `docs/ops/VPS-OS-TUNING.md` and PostgreSQL [kernel resources](https://www.postgresql.org/docs/current/kernel-resources.html).

## Files

| File | Purpose |
|------|---------|
| `sysctl-opencnpj.conf` | `vm.*`, `net.*`, `kernel.shm*` |
| `limits-postgres.conf` | `nofile` / `nproc` for `postgres` |
| `99-opencnpj-io-scheduler.rules` | `mq-deadline` on SSD |
| `fstab-postgres.example` | Mount options reference |
