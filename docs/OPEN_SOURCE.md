# Open Source

OpenCNPJ is a **fully open-source** platform for Brazilian CNPJ (company registry) public data.

## Principles

| Principle | Implementation |
|-----------|----------------|
| Open code | All application source in this repository |
| Open license | [MIT OR Apache-2.0](LICENSING.md) — your choice |
| Open docs | English documentation under `docs/` |
| Open governance | [CONTRIBUTING.md](CONTRIBUTING.md) + [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) |
| Open security | [SECURITY.md](SECURITY.md) — responsible disclosure |

## What is included

- **API** — Go REST service (`cmd/api`)
- **Importer** — high-throughput PostgreSQL COPY pipeline (`cmd/importer`)
- **Downloader** — RFB WebDAV fetcher (`cmd/downloader`)
- **Web portal** — React dashboard (`web/`)
- **Migrations** — PostgreSQL schema (`migrations/`)
- **CI/CD** — GitHub Actions (test, security, benchmark)

## What is not included

- Receita Federal CSV files (download separately via `make download`)
- Production secrets (use `.env` from `.env.example`)
- Hosted SaaS — self-deploy only

## Quick links

| Topic | Document |
|-------|----------|
| Getting started | [README.md](../README.md) |
| Architecture | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Data import | [IMPORT.md](IMPORT.md) |
| API security | [SECURITY.md](SECURITY.md) |
| Contributing | [CONTRIBUTING.md](CONTRIBUTING.md) |
| License | [LICENSING.md](LICENSING.md) |
| Changelog | [CHANGELOG.md](../CHANGELOG.md) |

## Commercial use

Permitted under MIT and Apache 2.0. No royalty, no copyleft. Attribution required (see license files).

## Data compliance

CNPJ records may contain personal data (partners, masked CPF). Deployers are responsible for LGPD/GDPR compliance in their jurisdiction. See [SECURITY.md](SECURITY.md) for masking and export policies.
