# Technical Debt Registry (DVT)

Sequential IDs for new features that still need production-grade automated tests.

| ID | Title | Status |
|----|-------|--------|
| DVT-01 | CNPJ open-data downloader (WebDAV) | open |
| DVT-02 | CNPJ sample importer (10% COPY pipeline) | open |
| DVT-03 | Typed Redis cache for search responses | open |
| DVT-04 | CSV export streaming without temp PL/pgSQL functions | open |
| DVT-05 | Fuzzy search pagination without full COUNT | open |

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
