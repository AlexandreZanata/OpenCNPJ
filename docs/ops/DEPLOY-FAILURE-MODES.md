# Deploy troubleshooting (CNPJ sync + VPS restore)

> Operator runbook for failures seen during RFB re-import, dump upload, and
> `RESTORE_ONLY` on the SaaS VPS. English only — no secrets.

Related: [MONTHLY-CNPJ-SYNC.md](MONTHLY-CNPJ-SYNC.md) · [SAAS-VPS-DEPLOY.md](SAAS-VPS-DEPLOY.md) ·
`scripts/repair_truncated_pipeline.sh` · `scripts/vps_first_deploy.sh`

## Symptom → cause → fix

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| API `404` `cnpj_not_found` for valid CNPJs | Incomplete dump (historical 512 MiB ZIP extract) or dump not restored | Re-download with fixed downloader (`scripts/redownload_truncated_rfb.sh`), full import, dump + `RESTORE_ONLY` |
| CSV files exactly **512 MiB** under `data/` | Old `maxZipMemberBytes` truncate | Clear `.downloaded` markers for those ZIPs; re-run downloader (`maxZipMemberBytes` is now 16 GiB) |
| `docker compose` / import: permission denied on docker.sock | Agent shell not in `docker` group | `sg docker -c '…'` or `newgrp docker`; pipeline scripts re-exec under `sg docker` when possible |
| Overnight import stops after download | Same docker.sock error | Re-run `scripts/run_full_import.sh` under `sg docker` |
| VPS `sha256sum -c` fails: path "No such file" | Checksum file embeds absolute PC path | Write checksum with **basename only** (`pc_to_vps_sync.sh`); re-upload `.sha256` |
| VPS disk full during staging restore | Old dump + staging DB + production DB | Free `/var/lib/opencnpj/incoming/*.dump` (keep `.zst` if needed); `docker builder prune`; drop `opencnpj_cnpj_old` after validation |
| API `500` `internal_error` … `statement timeout` (~2.5s/5s) | Missing hot-path indexes after import dropped PKs | Ensure `empresas_cnpj_basico_uidx` + `simples_cnpj_basico_uidx` (see `scripts/vps_create_indexes.sql` / `run_full_import.sh`) |
| `EXPLAIN` on `simples` shows **Seq Scan** | No unique index on `simples(cnpj_basico)` | `CREATE UNIQUE INDEX simples_cnpj_basico_uidx ON simples (cnpj_basico); ANALYZE simples;` |
| Lookup JOIN slow / timeout on `empresas` | PK on `empresas(cnpj_basico)` dropped by `drop_all_import_indexes.sh` and not recreated | `CREATE UNIQUE INDEX empresas_cnpj_basico_uidx ON empresas (cnpj_basico);` |
| `500` on `simples:` / `socios:` under load | Optional side queries timed out | Current API treats socios/simples as best-effort; still recreate indexes |
| `readyz` down / API inactive after restore | `RESTORE_ONLY` stops API and failed before restart | `systemctl start opencnpj-api`; check `/tmp` / journal for restore errors |
| Row counts local ≠ VPS | Dump/upload incomplete or restore rolled back | Compare `COUNT(*)` both sides; re-run upload + `DUMP_TAG=… RESTORE_ONLY=1` |
| Import log shows thousands of "errors" | Per-row parse/skip (invalid UF, etc.), not fatal | Check error rate ≪ total rows; investigate only if counts far below RFB expectations |

## Mandatory indexes after every full import / VPS restore

Import drops unique constraints for COPY speed. These **must** exist for public lookup:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS empresas_cnpj_basico_uidx ON empresas (cnpj_basico);
CREATE UNIQUE INDEX IF NOT EXISTS simples_cnpj_basico_uidx ON simples (cnpj_basico);
-- Plus UF partition indexes on estabelecimentos(cnpj_completo) via vps_create_indexes.sql
```

Canonical lists:

- Local: `scripts/run_full_import.sh` (index rebuild section)
- VPS: `scripts/vps_create_indexes.sql` (used by `vps_first_deploy.sh`)

## Operator checklist (post-restore)

1. `systemctl is-active opencnpj-api` → `active`
2. `curl -sf http://127.0.0.1:8081/readyz` → 200
3. Row counts ≥ previous dump (expect ~70M+ estabelecimentos for full RFB)
4. Indexes: `\di *empresas*cnpj*` and `\di *simples*cnpj*` in `opencnpj_cnpj`
5. Smoke: `GET /api/v1/cnpj/{cnpj}` with a known key for a previously truncated CNPJ (e.g. election committees starting `100000…`) and a bank matriz

## Related DVT

- **DVT-42** — RFB ZIP 512 MiB truncation (mitigated; keep download smoke gate open)
