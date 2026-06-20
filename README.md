# BUSCA-CNPJ-2026

High-performance Go API for Receita Federal open CNPJ data: search, import, export, and analytics.

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
| [AGENTS.md](AGENTS.md) | AI agent entry point + harness workflow |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Layers and packages |
| [docs/GLOSSARY.md](docs/GLOSSARY.md) | Domain terms (Empresa, Estabelecimento, CNAE…) |
| [docs/IMPORT.md](docs/IMPORT.md) | Bulk import from downloaded CSVs |
| [docs/SECURITY.md](docs/SECURITY.md) | Security controls |
| [docs/DVT.md](docs/DVT.md) | Technical debt / integration test backlog |
| [docs/COMMIT_CONVENTION.md](docs/COMMIT_CONVENTION.md) | Commit format |
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
| `GET /empresas/search` | Search by razão social / CNPJ básico |
| `GET /estabelecimentos/search` | Filter by CNAE, UF, município |
| `GET /estabelecimentos/:cnpj` | Lookup by 14-digit CNPJ |
| `POST /export/csv` | Filtered CSV export |
| `POST /export/phones` | Category phone export (CSV/TXT) |
| `GET /lookup/*` | Typeahead for export filters |
| `GET /analytics/summary` | Pre-aggregated stats |

Health: `GET /health`, `GET /readyz`, metrics: `GET /metrics`

## Agent harness

Rules load from `.cursor/rules/`. See [.cursor/README.md](.cursor/README.md).

```bash
pip install -r agent-harness/requirements.txt
./agent-harness/resolve-rules.sh api performance security
```

## License

MIT
