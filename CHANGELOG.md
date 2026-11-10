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

- Assistant in the product server: `serve --assistant-agg` adds `POST /api/v1/assistant/ask`
  and `/api/v1/assistant/export` as metered data routes (login key or session, plan checks,
  quotas, usage kind `assistant`, question in the search log); `GET /api/v1/platform`,
  `GET /api/v1/auth/usage` and `GET /api/v1/dominios/municipios/{uf}`; `serve --web` serves
  the front end.
- Interpretation of prospecting questions (`docs/intelligence/17-PROSPECTING.md`):
  search-style noun phrases, contact and lead lists, relative periods, capital without
  "social", company age, size class, legal forms, coordinated places, typo recovery for
  activities, twelve new sectors and the `prospecting-01` evaluation set (1,476 questions).
- New web front end (`web/`): assistant, filter search, CNPJ records, account with plan
  and usage, staff administration of accounts, plans and search limits; light and dark.

### Changed

- The interpreter executes a question only when every word outside a recognized place or
  activity is known (closed vocabulary): an unknown word or an unread number gets a
  clarification instead of being dropped. Answers list the filters they applied.
- The assistant, its assets and the engine executor moved from `opencnpj-lab` to
  `crates/opencnpj/src/assistant`; with `--web`, the service description moves from `/` to
  `/api/v1`.
- Rows that do not fit the layout are stored verbatim and counted instead of skipped;
  duplicate keys are kept and counted instead of deduplicated.
- 11-digit numeric tokens (CPFs embedded in MEI names) are not indexed.

### Removed

- Go API, PostgreSQL schema/migrations, Redis cache, SaaS admin/API keys, React portal,
  Docker/VPS deployment scripts. Preserved at tag `legacy-go-final` and branch
  `archive/pre-rust-rewrite`; see `docs/legacy/README.md`.
