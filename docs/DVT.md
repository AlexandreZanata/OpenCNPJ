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
