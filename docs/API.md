# HTTP API

```bash
opencnpj serve --snapshot data/2026-10.ocnpj --bind 127.0.0.1:8080
# or, with hot reload of <data-dir>/current (written by `opencnpj update`):
opencnpj serve --data-dir data
```

All responses are JSON. Errors keep the legacy shape:
`{"error": "cnpj_not_found", "message": "cnpj not found", "code": 404}`.
Values are the exact strings from the source; codes get a `*_descricao` sibling when a
reference table or the RFB layout defines them.

| Method & path | Description |
|---------------|-------------|
| `GET /api/v1/cnpj/{cnpj}` | Establishment by 14-character CNPJ (punctuation and lowercase accepted, check digits validated, numeric or alphanumeric) with company, partners and Simples. An 8-character root returns the company with a page of establishments (`estab_offset`, `estab_limit` ≤ 1000, default 100) and `total_estabelecimentos`. |
| `GET /api/v1/estabelecimentos/{cnpj}` | Legacy alias of the above |
| `GET /api/v1/empresas/{cnpj_basico}` | Company with a page of establishments (same parameters) |
| `GET /api/v1/search` | Name search and/or filters (below) |
| `GET /api/v1/empresas/search?razao_social=` | Legacy alias: search restricted to `razao_social` |
| `GET /api/v1/estabelecimentos/search` | Legacy alias of `/search` |
| `GET /api/v1/stats?group_by=uf&...` | Document counts per value of a filter field, restricted by the same parameters as `/search` |
| `GET /api/v1/export?format=ndjson\|csv&...` | Streaming export (disabled unless `--enable-export`) |
| `GET /api/v1/snapshot` | Competence, counts, section sizes, sources with SHA-256, snapshot SHA-256 |
| `GET /api/v1/dominios/{tabela}` | Reference table (`cnaes`, `municipios`, `naturezas`, `paises`, `qualificacoes`, `motivos`) |
| `GET /healthz`, `/health` | Liveness |
| `GET /readyz` | Snapshot loaded |
| `GET /metrics` | Prometheus text format |

## Search parameters

| Parameter | Meaning |
|-----------|---------|
| `q` | Name text, matched against `razao_social` and `nome_fantasia`. All tokens must match; the last token also matches as a prefix unless the query ends with a space. Accents, case and the stopwords `DA DAS DE DI DO DOS E` are ignored, and so are 11-digit numbers (CPFs are not searchable, see DATA_POLICY.md). |
| `field` | `any` (default), `razao_social`, `nome_fantasia` |
| `fuzzy=true` | Tokens with no exact match (≥ 4 chars) match within edit distance 1 (2 for ≥ 8 chars), bounded |
| `uf`, `municipio`, `cnae`, `cnae_secundaria`, `situacao`, `porte`, `natureza`, `simples`, `mei`, `matriz_filial` | Filters; comma-separated values are OR-ed, different parameters AND-ed. Codes as published by RFB (`situacao=02`, `municipio=7107`, `cnae=4711302`, `simples=S`, `matriz_filial=1`). Legacy names (`cnae_fiscal_principal`, `situacao_cadastral`, …) are accepted. |
| `limit` | 1–100 (default 20) |
| `offset` | ≤ 1000 |
| `cursor` | For filter-only queries: value of `next_cursor` from the previous page |

Ranking for text queries: shortest matching name first (by token count of the field that
matched), active establishments first, then CNPJ order. `total` is exact.
`expansion_truncated: true` means a prefix/fuzzy token matched more dictionary terms
than the limit (the most frequent ones were used) or the query had more than 8 tokens.

Response:

```json
{
  "total": 2,
  "results": [{ "cnpj": "00000000000191", "cnpj_basico": "00000000", "razao_social": "BANCO DO BRASIL SA",
                "nome_fantasia": "DIRECAO GERAL", "uf": "DF", "municipio": "9701", "municipio_descricao": "BRASILIA",
                "situacao_cadastral": "02", "situacao_cadastral_descricao": "ATIVA", "cnae_fiscal_principal": "6422100",
                "matriz_filial": "1" }],
  "expansion_truncated": false,
  "next_cursor": null,
  "snapshot": { "competence": "2026-10", "source_snapshot": "D61010" }
}
```

## Operational limits

| Flag | Default | Effect |
|------|---------|--------|
| `--max-concurrency` | 2 × cores | CPU-bound requests in flight; excess waits up to half the timeout, then `503 overloaded` |
| `--timeout-ms` | 5000 | `504 timeout` after this; the client gets an answer even if the work is still finishing |
| `--max-limit`, `--max-offset` | 100, 1000 | Page size and ranked-result depth |
| `--enable-export`, `--export-max-rows` | off, 100000 | Bulk streaming |
| `--hide-socios`, `--hide-contacts` | off | Data minimization (see [DATA_POLICY.md](DATA_POLICY.md)) |
| `--reload-secs` | 30 | Poll interval for `<data-dir>/current` |

Snapshot swaps are atomic: in-flight requests finish on the snapshot they started with.

## Migration from the legacy Go API

| Legacy | Now |
|--------|-----|
| `GET /api/v1/cnpj/{cnpj}` (14 digits) | Same path; also accepts alphanumeric CNPJ and 8-char roots |
| `GET /api/v1/estabelecimentos/{cnpj}` | Same path |
| `GET /api/v1/empresas/search?razao_social=` | Same path; trigram/FTS replaced by token + prefix matching |
| `GET /api/v1/estabelecimentos/search` (filters) | Same path; parameters as above |
| `POST /api/v1/export/csv`, `/export/phones` | `GET /api/v1/export` (opt-in) |
| `GET /api/v1/stats/uf`, `/stats/cnae`, `/analytics/summary` | `GET /api/v1/stats?group_by=uf|cnae|...` |
| `GET /api/v1/lookup/*` (typeahead) | `GET /api/v1/dominios/{tabela}`, `GET /api/v1/search?q=` |
| API keys, quotas, admin panel | Not part of the engine; use a gateway |
| `GET /health`, `/readyz`, `/metrics` | Same paths (+ `/healthz`) |

Response bodies changed: field names follow the RFB layout column names (as in
`FORMAT.md`) and every value is the original string.
