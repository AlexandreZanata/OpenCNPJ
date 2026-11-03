# Changelog

All notable changes to this project are documented in this file.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — Rust rewrite

### Added

- Rust workspace (`opencnpj-core`, `opencnpj-store`, `opencnpj-ingest`, `opencnpj`) and a
  single `opencnpj` binary: `download`, `update`, `rollback`, `build`, `verify`, `info`,
  `get`, `search`, `export`, `reconstruct`, `serve`, `bench`.
- `.ocnpj` single-file snapshot format v1.0 (see `docs/FORMAT.md`): company-clustered
  columnar blocks, lossless typed codecs, trained zstd dictionary, FST term dictionary,
  adaptive postings, per-block/section checksums.
- Alphanumeric CNPJ support (2026 layout) with check-digit validation.
- Semantic reconstruction check against the source ZIPs (`verify --sources`).
- Three build profiles (fast, balanced, compact) chosen from a measured experiment matrix.
- HTTP API with legacy-compatible lookup paths, search, stats, opt-in streaming export,
  snapshot info, health, readiness and Prometheus metrics; atomic hot reload.
- Monthly `update` with source-hash comparison, verification, atomic publish and rollback.
- Benchmark harness (JSON/CSV/Markdown) and scripts for experiments and baselines.

### Changed

- Rows that do not fit the layout are stored verbatim and counted instead of skipped;
  duplicate keys are kept and counted instead of deduplicated.
- 11-digit numeric tokens (CPFs embedded in MEI names) are not indexed.

### Removed

- Go API, PostgreSQL schema/migrations, Redis cache, SaaS admin/API keys, React portal,
  Docker/VPS deployment scripts. Preserved at tag `legacy-go-final` and branch
  `archive/pre-rust-rewrite`; see `docs/legacy/README.md`.
