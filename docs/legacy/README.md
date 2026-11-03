# Legacy implementation (Go + PostgreSQL + Redis + React)

The Rust rewrite replaced the original stack. The original code is preserved,
unchanged, in Git:

| Ref | Points to | Purpose |
|-----|-----------|---------|
| branch `archive/pre-rust-rewrite` | `68c6fe69b2cd3deb484916630b8cb6eaf9e879a6` | Read-only history branch |
| tag `legacy-go-final` (annotated) | same commit | Immutable marker |

Both refs are published on `origin`. Do not delete or force-push them.

## Restore and run the legacy system

```bash
git fetch origin --tags
git switch --detach legacy-go-final          # or: git worktree add ../opencnpj-legacy legacy-go-final
cp .env.example .env
docker compose up -d postgres redis
go run ./cmd/migrate
go run ./cmd/downloader                       # RFB WebDAV share → ./data
bash scripts/run_full_import.sh               # COPY import + index rebuild
go run ./cmd/api                              # → :8080
```

The legacy `docs/QUICKSTART.md`, `docs/IMPORT.md` and `docs/ops/*` describe the
full workflow (VPS deploy, SaaS keys, monthly `pg_dump` sync). They are kept in
the tag, not on this branch.

## What was kept here

| File | Why |
|------|-----|
| [benchmarks/](benchmarks/) | Historical measurements, used as the reference baseline |
| [DATA_FORMATS.md](DATA_FORMATS.md) | Legacy column notes for the RFB layouts |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Legacy layering, for contract comparison |
| [OPENAPI.yaml](OPENAPI.yaml) | Legacy public API contract (see `docs/API.md` for the migration map) |
| [ADR-001-use-pg-copy.md](ADR-001-use-pg-copy.md) | Original ingest decision |

## Legacy functionality inventory

| Area | Legacy behaviour | Status in the rewrite |
|------|------------------|-----------------------|
| Download | Nextcloud WebDAV share (`public.php/webdav`, public token), month dirs `YYYY-MM/` | Re-implemented in Rust (`opencnpj download`) |
| Import | Go parsers + PostgreSQL `COPY`, partitioned tables, index rebuild | Replaced by the `.ocnpj` snapshot builder |
| Dedupe | Duplicate CSV rows skipped (`internal/importer/dedupe.go`) | Rows are **kept** and counted; nothing is dropped silently |
| CNPJ lookup | `GET /api/v1/cnpj/{cnpj}`, `GET /api/v1/estabelecimentos/{cnpj}` | Same paths kept |
| Search | `GET /empresas/search` (pg_trgm / FTS), `GET /estabelecimentos/search` (filters) | `GET /api/v1/search` (inverted index + bitmaps); old paths aliased |
| Export | `POST /export/csv`, `POST /export/phones` | `GET /api/v1/export` (NDJSON/CSV streaming, opt-in) |
| Stats | Pre-aggregated tables (`/stats/uf`, `/stats/cnae`) | `GET /api/v1/stats?group_by=` computed from bitmaps |
| SaaS | API keys, quotas, admin panel, MFA (separate DB) | **Not ported.** Put a reverse proxy / gateway in front if needed |
| Cache | Redis response cache | Not needed: lookups are served from the snapshot directly |
| Web portal | React + Vite | Not ported in the first rewrite milestone |

## Known legacy bottlenecks (from the reports in `benchmarks/`)

- Cold CNPJ lookup p95 ≈ 7.5 s before cache warming (enrichment joins on a cold buffer cache).
- First search requests hit the 5 s Fiber timeout under DB load; warm path relied on Redis.
- Full import: 27 min 9 s for 217,259,823 rows (import + index rebuild), ~167k rows/s,
  under swap pressure on a 31 GB machine (`benchmarks/2026-06-24-full-import-i7-13620H-31GB.md`).
- ~116,746 parse/skip errors were counted but those rows were not stored.
- Operating cost: PostgreSQL (16 GB `shared_buffers`), Redis, pgBouncer, nginx.
