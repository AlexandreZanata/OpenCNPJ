# Technical Debt Registry (DVT)

Sequential IDs for new features that still need production-grade automated tests.

| ID | Title | Status |
|----|-------|--------|
| DVT-01 | CNPJ open-data downloader (WebDAV) | open |
| DVT-02 | CNPJ sample importer (10% COPY pipeline) | open |

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
