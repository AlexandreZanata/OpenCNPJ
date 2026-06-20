# BUSCA CNPJ — Web Portal

Enterprise React frontend for the Receita Federal CNPJ API (`/api/v1`).

## Stack

- React 19 + TypeScript + Vite
- TanStack Query (caching, stale-while-revalidate)
- React Router 7
- Tailwind CSS 4

## Domain terms

Uses glossary terms from `docs/GLOSSARY.md`: **Empresa**, **Estabelecimento**, **CNPJ**, **CNAE**, **SearchFilters**.

## Development

Requires [pnpm](https://pnpm.io) 9+ (`corepack enable`).

```bash
# Terminal 1 — API (port 8080)
go run ./cmd/api

# Terminal 2 — Web (port 5173, proxies /api → API)
cd web && pnpm install && pnpm dev
```

Open http://localhost:5173

## Environment

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_API_BASE_URL` | `/api/v1` | API prefix (use full URL in production) |

## Scripts

| Command | Purpose |
|---------|---------|
| `pnpm dev` | Dev server with HMR |
| `pnpm build` | Production build → `dist/` |
| `pnpm test` | Vitest unit tests |
| `pnpm preview` | Preview production build |

## Pages

| Route | Feature |
|-------|---------|
| `/` | Dashboard + quick CNPJ lookup |
| `/cnpj/:cnpj` | Estabelecimento detail by 14-digit CNPJ |
| `/empresas` | Empresa search (CNPJ básico, razão social) |
| `/estabelecimentos` | Estabelecimento search + filters |
| `/analytics` | CNAE / UF statistics |

## Architecture

```text
pages → TanStack Query → api/* → Go API /api/v1
components/ui (presentational)
utils (CNPJ validation, nullable unwrap)
```

Query limits respect API caps (`limit` ≤ 1000). Fuzzy text searches are debounced (500ms) to reduce load.
