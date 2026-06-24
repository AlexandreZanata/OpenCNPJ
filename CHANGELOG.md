# Changelog

All notable changes to this project are documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Enterprise open-source documentation (English)
- Dual licensing: MIT OR Apache-2.0
- Contributor Covenant Code of Conduct
- Security policy and tooling reference (English)
- Download progress bar (`[file/total] percentage`) in terminal
- `make download`, `make import-full`, `make download-and-import` one-command workflows
- Hardware auto-tuning (`scripts/lib/hardware_profile.sh`)
- docs/QUICKSTART.md, docs/HARDWARE.md, docs/benchmarks/HARDWARE-RTX4060-32GB.md

### Changed

- Project documentation translated to English
- Code comments and CLI messages standardized to English

## [0.1.0] - 2026-06-20

### Added

- Go API (`/api/v1`): empresa/estabelecimento search, CNPJ lookup, CSV export, phone export, analytics
- PostgreSQL 18 import pipeline (COPY, partitioned tables, pg_trgm)
- Redis response cache
- React web portal (`web/`)
- RFB WebDAV downloader
- CI: tests, security scans, benchmarks
