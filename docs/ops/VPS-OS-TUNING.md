# VPS OS tuning (OpenCNPJ plan 02 Phase 1)

Kernel and disk **example templates** for a DB-heavy VPS. Real configs stay on the host (gitignored under `deploy/vps/` if copied locally).

## Scope

| Area | Example template | Apply on |
|------|------------------|----------|
| `sysctl` | `deploy/vps/sysctl-opencnpj.conf.example` | VPS host |
| `ulimits` | `deploy/vps/limits-postgres.conf.example` | VPS host |
| I/O scheduler | `deploy/vps/99-opencnpj-io-scheduler.rules.example` | VPS host |
| Mount options | `deploy/vps/fstab-postgres.example` | VPS host |

Local Docker development **does not** require these changes.

## Workflow

```bash
# On VPS — copy, edit, apply (see deploy/vps/README.md)
sudo cp deploy/vps/sysctl-opencnpj.conf.example /etc/sysctl.d/99-opencnpj.conf
# ... edit, then sysctl --system
```

## Example starting values (~16 GB RAM)

| Parameter | Example | Rationale |
|-----------|---------|-----------|
| `vm.swappiness` | 1 | Keep Postgres buffer cache in RAM |
| `vm.dirty_ratio` | 10 | Bounded writeback under load |
| `kernel.shmmax` | 4 GB | Align with example `shared_buffers` (Phase 2) |
| `net.core.somaxconn` | 4096 | API + pgBouncer backlog |
| Postgres `nofile` | 65536 | Many connections via pooler |

## Validation

```bash
./scripts/opencnpj_advanced_phase1.sh http://localhost:8080
STRICT_VPS=1 ./scripts/opencnpj_advanced_phase1.sh http://localhost:8080  # after VPS apply
```

## References

- `deploy/vps/README.md` — copy/edit/apply checklist
- `.local/02-opencnpj-advanced-optimization/OFICIAL_SOURCES.md` — Linux / VPS links
