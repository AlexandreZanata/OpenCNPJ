# Technical Debt Registry (DVT)

Sequential IDs for new features that still need production-grade automated tests.

| ID | Title | Status |
|----|-------|--------|
| DVT-01 | CNPJ open-data downloader (WebDAV) | open |
| DVT-02 | CNPJ sample importer (10% COPY pipeline) | open |
| DVT-03 | Typed Redis cache for search responses | open |
| DVT-04 | CSV export streaming without temp PL/pgSQL functions | open |
| DVT-05 | Fuzzy search pagination without full COUNT | open |
| DVT-06 | Enterprise web portal (React) | open |
| DVT-07 | Pre-aggregated analytics stats tables | open |
| DVT-08 | Category phone export (CSV/TXT) | open |
| DVT-09 | Smart lookup typeahead for export filters | open |
| DVT-10 | Phone export date filter and export-all | open |
| DVT-11 | Download progress bar + one-command pipeline scripts | open |
| DVT-13 | Redis cache hit/miss Prometheus metrics | open |
| DVT-14 | Redis msgpack cache serialization | open |
| DVT-15 | Keyset cursor pagination for search | open |
| DVT-16 | PostgreSQL FTS for multi-word search | open |
| DVT-17 | Meilisearch indexer and search delegation | partial |
| DVT-18 | CI automated API perf validation gate | open |
| DVT-19 | OpenCNPJ plan 02 Phase 0 advanced baseline gate | open |
| DVT-27 | SaaS dual-database VPS mode | open |
| DVT-28 | SaaS API keys and usage tracking | open |
| DVT-29 | Public CNPJ API (sqlc + pgx) | open |
| DVT-30 | Admin auth + TOTP MFA | open |
| DVT-31 | Admin panel (server-rendered HTML) | open |
| DVT-34 | SaaS security hardening (Phase 9) | open |
| DVT-35 | SaaS production deploy runbook (Phase 10) | open |
| DVT-36 | Monthly CNPJ sync local PC → VPS (Phase 11) | open |
| DVT-37 | Data access & API performance stack (Phase 12) | open |

## DVT-01: CNPJ open-data downloader (WebDAV)

- **Scope**: `internal/downloader`, `cmd/downloader`
- **Added**: 2026-06-11
- **Needs**: integration test against mocked WebDAV; e2e download smoke with fixture ZIP
- **Status**: open

## DVT-02: CNPJ sample importer (10% COPY pipeline)

- **Scope**: `internal/importer`, `cmd/importer`, `cmd/migrate`
- **Added**: 2026-06-11
- **Needs**: integration test with testcontainers; full import benchmark on 100% data
- **Status**: open

## DVT-03: Typed Redis cache for search responses

- **Scope**: `internal/services/cache_typed.go`, `internal/services/search_service.go`
- **Added**: 2026-06-20
- **Needs**: integration test with Redis testcontainer; verify round-trip after API restart
- **Status**: open

## DVT-04: CSV export streaming without temp PL/pgSQL functions

- **Scope**: `internal/repository/csv_export.go`, `internal/repository/*_repo.go`
- **Added**: 2026-06-20
- **Needs**: integration test exporting filtered estabelecimentos against full dataset
- **Status**: open

## DVT-05: Fuzzy search pagination without full COUNT

- **Scope**: `internal/repository/csv_export.go` (search helpers), `internal/repository/*_repo.go`
- **Added**: 2026-06-20
- **Needs**: benchmark nome_fantasia/razao_social ILIKE on 71M rows with pg_trgm indexes
- **Status**: open

## DVT-06: Enterprise web portal (React)

- **Scope**: `web/`
- **Added**: 2026-06-20
- **Needs**: e2e tests (Playwright); auth/RBAC when API adds authentication
- **Status**: open

## DVT-07: Pre-aggregated analytics stats tables

- **Scope**: `migrations/000009_*`, `internal/repository/stats_repo.go`, `scripts/refresh_stats_aggregates.sh`
- **Added**: 2026-06-20
- **Needs**: integration test verifying refresh + `/analytics/summary` under 100ms; CI job after import
- **Status**: open

## DVT-08: Category phone export (CSV/TXT)

- **Scope**: `internal/exportcategory`, `internal/repository/phone_export.go`, `web/src/pages/PhoneExportPage.tsx`
- **Added**: 2026-06-20
- **Needs**: integration test export advocacia+SP; verify phone dedup and LGPD masking policy
- **Status**: open

## DVT-09: Smart lookup typeahead for export filters

- **Scope**: `internal/repository/lookup_repo.go`, `internal/services/lookup_service.go`, `internal/handlers/lookup_handler.go`, `web/src/components/search/SearchCombobox.tsx`
- **Added**: 2026-06-20
- **Needs**: integration test for `/lookup/*` endpoints with live DB; e2e typeahead on Phone Export page
- **Status**: open

## DVT-10: Phone export date filter and export-all

- **Scope**: `internal/repository/phone_export_filters.go`, `web/src/pages/PhoneExportPage.tsx`, `web/src/components/ui/ProgressBar.tsx`
- **Added**: 2026-06-20
- **Needs**: integration test export with date range; verify export_all omits LIMIT on large filter sets
- **Status**: open

## DVT-11: Download progress bar + one-command pipeline scripts

- **Scope**: `internal/downloader/progress.go`, `scripts/download_latest.sh`, `scripts/download_and_import.sh`, `scripts/lib/hardware_profile.sh`
- **Added**: 2026-06-24
- **Needs**: e2e test with mocked HTTP Content-Length; CI smoke for hardware_profile.sh
- **Status**: open

## DVT-12: Full empresa aggregate in search endpoints

- **Scope**: `internal/models/aggregate.go`, `internal/repository/aggregate_build.go`, `internal/services/search_service.go`, `web/src/pages/EmpresaSearchPage.tsx`, `web/src/pages/EstabelecimentoSearchPage.tsx`
- **Added**: 2026-06-24
- **Needs**: integration test `/empresas/search` and `/estabelecimentos/search` return empresa + branches + sócios + simples; e2e UI renders all fields
- **Status**: open

## DVT-13: Redis cache hit/miss Prometheus metrics

- **Scope**: `internal/services/cache_metrics.go`, `internal/services/cache_service.go`, `internal/services/cache_typed.go`
- **Added**: 2026-06-24
- **Needs**: integration test with Redis testcontainer verifying hit/miss counters increment per key prefix under load
- **Status**: open

## DVT-14: Redis msgpack cache serialization

- **Scope**: `internal/services/cache_serialize.go`, `internal/services/cache_service.go`
- **Added**: 2026-06-24
- **Needs**: integration test verifying msgpack round-trip and legacy JSON cache key compatibility after deploy
- **Status**: open

## DVT-15: Keyset cursor pagination for search

- **Scope**: `internal/repository/pagination.go`, `internal/handlers/search_pagination.go`, `internal/models/dto.go`
- **Added**: 2026-06-24
- **Needs**: integration test deep-page cursor vs offset latency; e2e UI cursor navigation
- **Status**: open

## DVT-16: PostgreSQL FTS for multi-word search

- **Scope**: `migrations/000012_fts_search_columns.up.sql`, `internal/repository/search_query.go`
- **Added**: 2026-06-24
- **Needs**: integration test multi-word `razao_social` / `nome_fantasia` ranking quality vs trigram
- **Status**: open

## DVT-17: Meilisearch indexer and search delegation

- **Scope**: `internal/meilisearch/`, `cmd/meilisearch-index`, `internal/services/search_meili.go`, `cmd/importer`
- **Added**: 2026-06-24
- **Needs**: e2e test with Meilisearch testcontainer; full-dataset index benchmark; handler integration test with `meilisearch.enabled: true`
- **Status**: partial (code complete, disabled by default)

## DVT-18: CI automated API perf validation gate

- **Scope**: `scripts/api_perf_validation.sh`, `internal/perfvalidation/stats.go`
- **Added**: 2026-06-24
- **Needs**: GitHub Actions job running validation script against docker-compose stack on PR
- **Status**: open

## DVT-19: OpenCNPJ plan 02 Phase 0 advanced baseline gate

- **Scope**: `scripts/opencnpj_advanced_phase0.sh`, `scripts/opencnpj_advanced_baseline.sh`, `internal/perfvalidation/phase0_gate.go`
- **Added**: 2026-06-25
- **Needs**: CI job + VPS k6 baseline on 150M-row staging; wire `opencnpj_advanced_phase0.sh` into workflow
- **Status**: open

## DVT-20: OpenCNPJ plan 02 Phase 1 VPS OS tuning gate

- **Scope**: `deploy/vps/`, `scripts/opencnpj_advanced_phase1.sh`, `internal/perfvalidation/phase1_gate.go`, `docs/ops/VPS-OS-TUNING.md`
- **Added**: 2026-06-25
- **Needs**: Apply on 16 GB VPS; `STRICT_VPS=1` gate in staging workflow; post-reboot verification
- **Status**: open

## DVT-21: OpenCNPJ plan 02 Phase 2 PostgreSQL 16 GB profile gate

- **Scope**: `deploy/vps/*.example`, `scripts/opencnpj_advanced_phase2.sh`, `scripts/vps_analyze_search_tables.sh`, `internal/perfvalidation/phase2_gate.go`, `docs/ops/VPS-POSTGRESQL.md`
- **Added**: 2026-06-25
- **Needs**: Apply on VPS; `STRICT_VPS=1` SHOW GUC verification; post-apply k6 delta vs Phase 0 baseline
- **Status**: open

## DVT-22: OpenCNPJ plan 02 Phase 3 Ristretto L1 cache

- **Scope**: `internal/cache/l1/`, `internal/services/cache_layers.go`, `scripts/opencnpj_advanced_phase3.sh`, `internal/perfvalidation/phase3_gate.go`
- **Added**: 2026-06-25
- **Needs**: k6 steady-load L1 hit rate > 70% on CNPJ path; integration test with Redis testcontainer
- **Status**: open

## DVT-23: OpenCNPJ plan 02 Phase 4 materialized views

- **Scope**: `migrations/000013_*`, `internal/repository/stats_repo.go`, `internal/repository/lookup_repo.go`, `scripts/opencnpj_advanced_phase4.sh`, `docs/ops/MATERIALIZED-VIEWS.md`
- **Added**: 2026-06-25
- **Needs**: integration test verifying CONCURRENTLY refresh + `/analytics/summary` p99 < 20ms; pg_cron on VPS
- **Status**: open

## DVT-24: OpenCNPJ plan 02 Phase 5 Meilisearch selective index

- **Scope**: `internal/meilisearch/selective.go`, `internal/meilisearch/indexer.go`, `cmd/meilisearch-index`, `scripts/opencnpj_advanced_phase5.sh`, `docs/ops/MEILISEARCH-SELECTIVE.md`
- **Added**: 2026-06-25
- **Needs**: full ~20M doc index on staging; text search p99 < 80 ms k6 gate; importer post-import sync e2e
- **Status**: open

## DVT-25: OpenCNPJ plan 02 Phase 6 UF LIST partitioning

- **Scope**: `migrations/000014_*`, `internal/partition/`, `scripts/opencnpj_advanced_phase6.sh`, `scripts/explain_uf_partition_pruning.sh`, `docs/ops/UF-PARTITIONING.md`
- **Added**: 2026-06-25
- **Needs**: off-peak apply on 150M-row VPS; STRICT EXPLAIN gate in CI; post-migrate k6 uf_search p99 < 80 ms
- **Status**: open

## DVT-26: CSV bulk export 500k row cap

- **Scope**: `internal/repository/export_limits.go`, `internal/handlers/export_handler.go`, `scripts/benchmark_export.sh`, `docs/benchmarks/2026-06-25-export-throughput.md`
- **Added**: 2026-06-25
- **Needs**: e2e export 500k on VPS with timeout/load test; UI `ExportPanel` limit parity
- **Status**: open

## DVT-27: SaaS dual-database VPS mode

- **Scope**: `internal/config/saas.go`, `internal/database/postgres_dual.go`, `migrations/saas/`, `config/config.saas.example.yaml`, `cmd/migrate --saas`
- **Added**: 2026-07-07
- **Needs**: integration test with testcontainers (two Postgres DBs); e2e `readyz` + `public_api_only` route gate on staging VPS
- **Status**: open

## DVT-28: SaaS API keys and usage tracking

- **Scope**: `internal/saas/`, `internal/db/saas/`, `db/queries/saas/`, `sqlc.yaml`, `migrations/saas/000003_*`
- **Added**: 2026-07-07
- **Needs**: e2e gate on staging VPS with real Redis; verify `api_usage_daily` flush after 5 min; k6 load test per-client rate limit
- **Status**: open

## DVT-29: Public CNPJ API (sqlc + pgx)

- **Scope**: `internal/cnpj/`, `internal/handlers/cnpj_handler.go`, `db/queries/cnpj/`, `internal/database/cnpj_pgx.go`
- **Added**: 2026-07-07
- **Needs**: staging VPS e2e with real CNPJ data; k6 warm-cache p95 gate; monthly dump regression on `idx_estabelecimentos_cnpj_completo`
- **Status**: open

## DVT-30: Admin auth + TOTP MFA

- **Scope**: `internal/adminauth/`, `cmd/admin-bootstrap`, `db/queries/saas/admin_auth.sql`, `migrations/saas/000004_admin_seed`
- **Added**: 2026-07-07
- **Needs**: staging VPS e2e with real Redis + RS256 keys; verify brute-force lockout and refresh rotation under load
- **Status**: open

## DVT-31: Admin panel (server-rendered HTML)

- **Scope**: `internal/handlers/admin/`, `internal/templates/admin/`, `internal/static/admin/`, `db/queries/saas/admin_panel.sql`
- **Added**: 2026-07-07
- **Needs**: staging VPS browser e2e (create client/key, usage after API call); `ps aux` RSS gate < 80 MB idle
- **Status**: open

## DVT-32: OpenCNPJ plan 02 Phase 7 CNAE HASH sub-partitions

- **Scope**: `migrations/000016_*`, `internal/partition/cnae_hash.go`, `scripts/opencnpj_advanced_phase7.sh`, `scripts/explain_cnae_uf_partition_pruning.sh`, `docs/ops/CNAE-PARTITIONING.md`
- **Added**: 2026-07-07
- **Needs**: off-peak apply on 150M-row VPS; EXPLAIN before/after on production-size copy; post-migrate k6 cnae+uf_search p99 < 80 ms
- **Status**: open

## DVT-33: SaaS public API documentation (Phase 8)

- **Scope**: `docs/api/*`, `internal/apidocs/`, `scripts/api_docs_gate.sh`, admin panel API docs link
- **Added**: 2026-07-07
- **Needs**: QUICKSTART e2e on staging VPS with test key; Redoc `/docs` behind nginx in non-prod only
- **Status**: open

## DVT-34: SaaS security hardening (Phase 9)

- **Scope**: `internal/saas/hash_compare.go`, `internal/middleware/metrics_auth.go`, `internal/handlers/admin/csrf.go`, `internal/adminauth/audit/`, `scripts/security_hardening_gate.sh`, `docs/SECURITY.md`
- **Added**: 2026-07-07
- **Needs**: staging VPS pen-test (no auth bypass on `/api/v1/cnpj/*`); nginx TLS/HSTS verification; audit log e2e after admin actions
- **Status**: open

## DVT-35: SaaS production deploy runbook (Phase 10)

- **Scope**: `docs/ops/DEPLOY-RUNBOOK.md`, `scripts/saas_smoke.sh`, `scripts/saas_deploy_gate.sh`, `scripts/build_opencnpj_api.sh`, `deploy/saas/redis-opencnpj.conf.example`, `deploy/saas/rollback.example.sh`
- **Added**: 2026-07-07
- **Needs**: full empty-VPS → working API path executed once on staging; smoke with production API key after first client created
- **Status**: open

## DVT-36: Monthly CNPJ sync local PC → VPS (Phase 11)

- **Scope**: `docs/ops/MONTHLY-CNPJ-SYNC.md`, `deploy/saas/monthly-cnpj-sync.example.sh`, `deploy/saas/grant-reader.sql.example`, `scripts/saas_monthly_cnpj_sync_gate.sh`
- **Added**: 2026-07-07
- **Needs**: full operator cycle on staging VPS with real multi-GB dump; verify `opencnpj_saas` row counts unchanged; CNPJ lookup smoke after swap
- **Status**: open

## DVT-37: Data access & API performance stack (Phase 12)

- **Scope**: `docs/ops/DATA-ACCESS-PERFORMANCE.md`, `sqlc.yaml`, `internal/database/cnpj_pgx.go`, `internal/cnpj/service.go`, `scripts/saas_data_access_gate.sh`
- **Added**: 2026-07-07
- **Needs**: staging VPS p95 CNPJ lookup < 150 ms cache miss; API key middleware < 5 ms p95; post-restore EXPLAIN on production-size copy
- **Status**: open
