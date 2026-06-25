# VPS OS tuning (OpenCNPJ plan 02 Phase 1)

Production kernel and disk settings for a **16 GB RAM** VPS running PostgreSQL 18, Redis, and the Go API.

## Scope

| Area | Artifact | Apply on |
|------|----------|----------|
| `sysctl` | `deploy/vps/sysctl-opencnpj.conf` | VPS host |
| `ulimits` | `deploy/vps/limits-postgres.conf` | VPS host |
| I/O scheduler | `deploy/vps/99-opencnpj-io-scheduler.rules` | VPS host |
| Mount options | `deploy/vps/fstab-postgres.example` | VPS host |

Local Docker development **does not** require these changes.

## Key values

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `vm.swappiness` | 1 | Keep Postgres buffer cache in RAM |
| `vm.dirty_ratio` | 10 | Bounded writeback under load |
| `kernel.shmmax` | 4 GB | Matches production `shared_buffers` (Phase 2) |
| `net.core.somaxconn` | 4096 | API + pgBouncer backlog |
| Postgres `nofile` | 65536 | Many connections via pooler |

## Validation

```bash
./scripts/opencnpj_advanced_phase1.sh http://localhost:8080
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase1.sh http://localhost:8080  # after VPS apply
```

Gate: artifact checks pass; light k6 load does not spike swap usage beyond threshold.

## References

- `deploy/vps/README.md` — apply steps
- `.local/02-opencnpj-advanced-optimization/OFICIAL_SOURCES.md` — Linux / VPS links
- PostgreSQL kernel resources — huge pages (optional)
