# Monthly CNPJ sync (Phase 11)

> Local PC imports RFB data; VPS receives a `pg_dump` restore into **`opencnpj_cnpj` only**.  
> **`opencnpj_saas` is never modified** during monthly sync.

Frequency: **monthly** when Receita Federal publishes new open data.

## Process summary

| Where | What |
|-------|------|
| **Local PC** | Download RFB → import → validate → `pg_dump` |
| **Transfer** | Encrypted upload from PC to VPS |
| **VPS** | Restore into `opencnpj_cnpj` only — SaaS DB untouched |

## What never changes on VPS during sync

- `opencnpj_saas` database (API keys, clients, admin, usage history)
- Redis SaaS keys (optional flush of `cnpj:*` cache only)
- nginx / systemd / API binary

## Repo artifacts

| File | Purpose |
|------|---------|
| `deploy/saas/monthly-cnpj-sync.example.sh` | Operator script: dump, upload, restore, rollback |
| `deploy/saas/grant-reader.sql.example` | Re-apply `opencnpj_reader` grants after swap |
| `scripts/saas_monthly_cnpj_sync_gate.sh` | CI/local gate for templates + optional Docker swap test |

## Local PC workflow (import workstation)

### Step 1 — Download

```bash
cd /path/to/BUSCA-CNPJ-2026
./scripts/download_and_import.sh --download-only
# or: go run ./cmd/downloader
```

### Step 2 — Import into local Postgres

```bash
docker compose up -d postgres
./scripts/run_full_import.sh
```

### Step 3 — Apply CNPJ migrations (if not already in dump pipeline)

```bash
DATABASE_URL='postgres://receita_user:...@localhost:5434/receita_db' go run ./cmd/migrate up
./scripts/refresh_stats_aggregates.sh
```

### Step 4 — Validate before dump

```bash
psql "$LOCAL_DATABASE_URL" -c "SELECT count(*) FROM estabelecimentos"
psql "$LOCAL_DATABASE_URL" -c "SELECT pg_get_partkeydef('estabelecimentos'::regclass)"
```

Record counts in a local operator log (not committed): `~/opencnpj-sync-YYYY-MM.log`.

### Step 5 — Dump and compress

```bash
export LOCAL_DATABASE_URL='postgres://receita_user:...@localhost:5434/receita_db'
./deploy/saas/monthly-cnpj-sync.example.sh local-dump
```

Or manually:

```bash
export DUMP_FILE="opencnpj_cnpj_$(date +%Y%m).dump"
pg_dump -Fc --no-owner --no-acl -f "$DUMP_FILE" "$LOCAL_DATABASE_URL"
zstd -T0 -19 -f "$DUMP_FILE"
sha256sum "${DUMP_FILE}.zst" | tee "${DUMP_FILE}.zst.sha256"
```

## Transfer PC → VPS

```bash
export VPS_HOST=YOUR_VPS_IP
export VPS_USER=root
export REMOTE_INCOMING=/var/lib/opencnpj/incoming
./deploy/saas/monthly-cnpj-sync.example.sh upload
```

Use SSH key auth. **Never** commit VPS credentials.

## VPS restore workflow

### Pre-restore checklist

- [ ] Announce maintenance window (API can stay up with stale data or brief read-only)
- [ ] Verify dump checksum on VPS: `sha256sum -c opencnpj_cnpj_YYYY-MM.dump.zst.sha256`
- [ ] Confirm free disk ≥ 2× database size
- [ ] Optional backup: `pg_dump -Fc opencnpj_cnpj > backup_pre_YYYY-MM.dump`

### One-time VPS setup

```bash
sudo mkdir -p /var/lib/opencnpj/incoming /etc/opencnpj
sudo cp deploy/saas/grant-reader.sql.example /etc/opencnpj/grant-reader.sql
```

### Restore (swap strategy — recommended)

```bash
export DUMP_TAG=$(date +%Y%m)   # or YYYY-MM of the dump file
sudo -E ./deploy/saas/monthly-cnpj-sync.example.sh vps-restore
```

The script:

1. Decompresses the dump in `/var/lib/opencnpj/incoming/`
2. Stops `opencnpj-api`
3. Restores into `opencnpj_cnpj_new`
4. Validates row counts on staging
5. Renames `opencnpj_cnpj` → `opencnpj_cnpj_old`, staging → `opencnpj_cnpj`
6. Re-applies `/etc/opencnpj/grant-reader.sql`
7. Runs `ANALYZE`
8. Flushes Redis keys matching `cnpj:*` only
9. Starts `opencnpj-api`

### Drop old database (after 24h validation)

```bash
sudo ./deploy/saas/monthly-cnpj-sync.example.sh vps-drop-old
```

## Rollback

If new data is bad:

```bash
sudo ./deploy/saas/monthly-cnpj-sync.example.sh vps-rollback
```

SaaS DB and API keys remain unaffected.

## Gate (CI / local)

```bash
./scripts/saas_monthly_cnpj_sync_gate.sh           # templates + unit tests
./scripts/saas_monthly_cnpj_sync_gate.sh --docker  # dump/restore/swap integration
go test ./deploy/saas/... -run TestMonthly -count=1
```

## Phase 11 operator gate

- [ ] Full cycle executed once: local import → dump → VPS restore → CNPJ lookup OK
- [ ] `opencnpj_saas` row counts unchanged after restore
- [ ] Documented in operator log with date + row counts + dump checksum

## Related docs

- [SAAS-VPS-DEPLOY.md](SAAS-VPS-DEPLOY.md) — SaaS overview
- [DUAL-DATABASE-VPS.md](DUAL-DATABASE-VPS.md) — two-database model
- [IMPORT.md](../IMPORT.md) — local import pipeline
- [DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md) — first deploy (includes first CNPJ restore)
