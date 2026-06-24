# OpenCNPJ Platform

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE-2.0)

High-performance **open-source** platform for Brazilian Receita Federal CNPJ public data: bulk import, search API, CSV export, and analytics.

> **License:** [MIT OR Apache-2.0](docs/LICENSING.md) — your choice.

## Stack

| Component | Version |
|-----------|---------|
| Go | 1.21+ |
| PostgreSQL | 18.4 (partitioned tables, pg_trgm) |
| Redis | 7 (response cache) |
| React portal | `web/` (Vite + TanStack Query) |

## Quick start

```bash
cp .env.example .env
docker compose up -d postgres redis
go run ./cmd/migrate
go run ./cmd/api          # API → :8080
cd web && pnpm dev        # UI  → :5173
```

## Documentation

| Doc | Purpose |
|-----|---------|
| [docs/README.md](docs/README.md) | **Documentation index** |
| [docs/OPEN_SOURCE.md](docs/OPEN_SOURCE.md) | Open-source scope and principles |
| [docs/LICENSING.md](docs/LICENSING.md) | MIT OR Apache-2.0 dual license |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | How to contribute |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Community standards |
| [SECURITY.md](docs/SECURITY.md) | Security policy |
| [AGENTS.md](AGENTS.md) | AI agent entry point |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Layers and packages |
| [docs/GLOSSARY.md](docs/GLOSSARY.md) | Domain terms |
| [docs/IMPORT.md](docs/IMPORT.md) | Bulk import from CSVs |
| [CHANGELOG.md](CHANGELOG.md) | Release history |
| [web/README.md](web/README.md) | Frontend portal |

## Makefile targets

```bash
make setup              # docker + deps
make test               # unit tests
make import-sample      # 10% sample import
make benchmark-all-approaches
bash scripts/run_full_import.sh   # 100% import + index rebuild
make web-dev            # frontend dev server
```

## API (prefix `/api/v1`)

| Endpoint | Description |
|----------|-------------|
| `GET /empresas/search` | Search by legal name / CNPJ root |
| `GET /estabelecimentos/search` | Filter by CNAE, state, municipality |
| `GET /estabelecimentos/:cnpj` | Lookup by 14-digit CNPJ |
| `POST /export/csv` | Filtered CSV export |
| `POST /export/phones` | Category phone export (CSV/TXT) |
| `GET /lookup/*` | Typeahead for export filters |
| `GET /analytics/summary` | Pre-aggregated stats |

Health: `GET /health`, `GET /readyz`, metrics: `GET /metrics`

## Open source

This project is 100% open source. See [docs/OPEN_SOURCE.md](docs/OPEN_SOURCE.md).

## Agent harness

Rules load from `.cursor/rules/`. See [.cursor/README.md](.cursor/README.md).

```bash
pip install -r agent-harness/requirements.txt
./agent-harness/resolve-rules.sh api performance security
```

## License

Licensed under **MIT OR Apache-2.0**, at your option. See [LICENSE](LICENSE) and [LICENSE-APACHE-2.0](LICENSE-APACHE-2.0).
