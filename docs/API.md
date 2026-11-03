# HTTP API

```bash
opencnpj serve --snapshot data/snapshots/2026-10-balanced.ocnpj          # public API :8080, admin :8081
opencnpj serve --data-dir data --bind 0.0.0.0:8080 --admin-bind 127.0.0.1:8081
```

With `--data-dir`, the server follows `<data-dir>/current` (written by `opencnpj update`
and by the admin "activate" action) and can run admin jobs.

**This build has no authentication and no request limits by default**: page size up to
10,000, no offset limit, export enabled with no row cap, no rate limiting. Only a 60 s
per-request deadline and a concurrency bound (4 × cores) remain, both configurable.
Keep the admin listener on a private address, and put a gateway in front before exposing
the public API (see [DATA_POLICY.md](DATA_POLICY.md)).

All responses are JSON. Errors keep the legacy shape:
`{"error": "cnpj_not_found", "message": "cnpj not found", "code": 404}`.
Values are the exact strings from the source; codes get a `*_descricao` sibling when a
reference table or the RFB layout defines them.

## Endpoints

| Method & path | Description |
|---------------|-------------|
| `GET /api/v1/cnpj/{cnpj}` | Establishment by 14-character CNPJ (punctuation and lowercase accepted, check digits validated, numeric or alphanumeric) with company, partners and Simples. An 8-character root returns the company with a page of establishments (`estab_offset`, `estab_limit`, default 100) and `total_estabelecimentos`. |
| `GET /api/v1/estabelecimentos/{cnpj}` | Legacy alias of the above |
| `GET /api/v1/empresas/{cnpj_basico}` | Company with a page of establishments (same parameters) |
| `GET /api/v1/cpf/{cpf}?limit=` | Companies linked to a CPF: `titular` (exact — the CPF is part of an individual-entrepreneur/MEI name) and `socios` (possible — partners/representatives whose masked CPF shows the same 6 middle digits, with the matching partner rows so the name can be checked) |
| `GET /api/v1/search` (alias `/api/v1/busca`) | Name/CNPJ/CPF search and filters (below) |
| `GET /api/v1/empresas/search?razao_social=` | Legacy alias: search restricted to `razao_social` |
| `GET /api/v1/estabelecimentos/search` | Legacy alias of `/search` |
| `GET /api/v1/stats?group_by=` | Counts per value of a filter field, or per year with `group_by=ano_<date column>`, restricted by the same parameters as `/search` |
| `GET /api/v1/export?format=ndjson\|csv&max=` | Streaming export of every match (same parameters as `/search`) |
| `GET /api/v1/campos` | Machine-readable list of every parameter, filter, range and sort option |
| `GET /api/v1/snapshot` | Competence, counts, section sizes, sources with SHA-256, snapshot SHA-256 |
| `GET /api/v1/distribution` | Snapshot files offered for download, with sizes and SHA-256 |
| `GET /downloads/{file}` | Download a snapshot / checksum / build report (supports `Range`, resumable) |
| `GET /api/v1/dominios`, `/api/v1/dominios/{tabela}` | Reference tables (`cnaes`, `municipios`, `naturezas`, `paises`, `qualificacoes`, `motivos`) |
| `GET /healthz`, `/health`, `/readyz` | Liveness / readiness |
| `GET /metrics` | Prometheus: per-route request counts, latency histograms, status classes, cache, RSS |

## Search parameters

Comma-separated values of one parameter are OR-ed; different (or repeated) parameters
are AND-ed. Responses include `total` (exact), `results`, `fuzzy_used`,
`expansion_truncated`, `next_cursor` and `took_ms`.

| Parameter | Meaning |
|-----------|---------|
| `q` (aliases `nome`, `razao_social`, `nome_fantasia`) | Name tokens matched against razão social and nome fantasia (all must match; the last is also a prefix unless the query ends with a space; accents, case and `DA DAS DE DI DO DOS E` ignored). A value that is only a CNPJ or CNPJ root (8–14 digits, not 11) is a CNPJ search; an 11-digit number is an owner CPF. |
| `field` | `any` (default), `razao_social`, `nome_fantasia` |
| `fuzzy` | `auto` (default: when fewer than 5 exact hits, retry with typo tolerance and keep it if it finds more), `true`, `false`. Typo tolerance: edit distance 1 (2 for tokens ≥ 8 chars) plus swapped adjacent letters, most frequent candidates first, bounded |
| `cnpj` | CNPJ prefix, 1–14 characters (`33000167`, `33.000.167/0001`, alphanumeric accepted) |
| `cpf` | Owner CPF in MEI/EI names (exact) **or** masked partner/representative CPF (6 middle digits — possible match) |
| `cpf_titular` | Owner CPF only (exact) |
| `socio_cnpj`, `socio_cpf` (= `socio_documento`) | Partner document: 14-digit CNPJ (exact) or CPF (masked match) |
| `uf`, `municipio` | State, RFB municipality code (`7107`) |
| `cnae`, `cnae_secundaria`, `cnae_qualquer` | Main, secondary, or either CNAE: 7 digits, a prefix (2 digits = divisão, 3 = grupo, 5 = classe; `47` or `47*`), or a section letter `A`–`U` |
| `situacao`, `motivo`, `matriz_filial`, `porte`, `natureza` | RFB codes (`situacao=02`, `matriz_filial=1`, `porte=01`, `natureza=2135` or prefix `21`) |
| `simples`, `mei` | `S` / `N` |
| `mei_ativo`, `simples_ativo` | `true`: opted in and never excluded |
| `cep` | 8 digits or a prefix (`01310`) |
| `ddd` | Area code of the first phone |
| `contato`, `tem_email`, `tem_telefone` | `contato=email\|telefone`; aliases `tem_email=true`, `tem_telefone=true` |
| `<column>_de`, `<column>_ate` | Inclusive range on `data_inicio_atividade`, `data_situacao_cadastral`, `data_opcao_simples`, `data_exclusao_simples`, `data_opcao_mei`, `data_exclusao_mei` (dates `YYYY-MM-DD`, `YYYYMMDD` or a year) and `capital_social` (reais; `_min`/`_max` also accepted) |
| `<column>_vazio=true` | Documents without a value in that column (e.g. never excluded) |
| `ordenar` | `<column>:asc\|desc` (e.g. `data_inicio_atividade:desc` = newest first). Default: relevance for text, CNPJ order for filters |
| `limit`, `offset` | Page size (≤ `--max-limit`, default 20) and offset (no limit by default) |
| `cursor` | Filter-only queries: `next_cursor` from the previous page |

Ranking for text queries: shortest matching name first (by token count of the field that
matched), active establishments first, then CNPJ order.

Examples:

```bash
curl 'localhost:8080/api/v1/search?q=padaria&uf=SP&municipio=7107&situacao=02'
curl 'localhost:8080/api/v1/search?cnae=G&data_inicio_atividade_de=2026-01-01&ordenar=data_inicio_atividade:desc'
curl 'localhost:8080/api/v1/search?mei_ativo=true&municipio=7107&tem_telefone=true&limit=1000'
curl 'localhost:8080/api/v1/search?cnae_qualquer=6201501&capital_social_min=1000000'
curl 'localhost:8080/api/v1/search?q=supremercado'                     # typo → fuzzy_used: true
curl 'localhost:8080/api/v1/cpf/123.456.789-01'
curl 'localhost:8080/api/v1/stats?group_by=ano_data_inicio_atividade&uf=SP&cnae=56'
curl -o snap.ocnpj -C - 'localhost:8080/downloads/2026-10-balanced-1.2.ocnpj'
```

## Server flags

| Flag | Default | Effect |
|------|---------|--------|
| `--bind` | `127.0.0.1:8080` | Public API |
| `--admin-bind` | `127.0.0.1:8081` | Admin suite + public API (empty string disables) |
| `--max-concurrency` | 4 × cores | CPU-bound requests in flight; excess waits, then `503 overloaded` |
| `--timeout-ms` | 60000 | `504 timeout` after this (0 = none) |
| `--max-limit`, `--max-offset` | 10000, unlimited | Page size and depth |
| `--disable-export`, `--export-max-rows` | export on, 0 = unlimited | Bulk streaming |
| `--postings-cache-mib` | 512 | Decoded postings kept in memory |
| `--hide-socios`, `--hide-contacts` | off | Data minimization |
| `--reload-secs` | 30 | Poll interval for `<data-dir>/current` |

## Admin suite (`http://<admin-bind>/admin`)

A single-page dashboard (no external assets) plus a JSON API, on its own listener with
**no authentication**:

| Tab / endpoint | What it does |
|----------------|--------------|
| Overview — `GET /admin/api/overview` | Uptime, requests, req/s, p95 per route, timeouts, RSS (anonymous vs. mmap), CPU, cache hit rate, snapshot sections and counts, live 5-minute charts |
| Routes — `GET /admin/api/metrics?seconds=` | Per-route count, mean, p50/p90/p95/p99/max (log-histogram, ~9% resolution), 4xx/5xx; per-second series (15 min) |
| Search — `GET /admin/api/explain?<search params>` | Runs a query and returns the plan (documents per filter and range alone, candidates, matches per field, fuzzy/truncation) and timings per stage (candidates, search, record fetch, JSON) |
| Snapshots — `GET /admin/api/snapshots`, `POST /admin/api/snapshots/activate {name}`, `POST /admin/api/reload` | List files with SHA-256, switch the served snapshot atomically (updates `current` with `--data-dir`), reload |
| Index — `GET /admin/api/index` | Terms and postings bytes per index field |
| Jobs — `GET/POST /admin/api/jobs`, `GET /admin/api/jobs/{id}/log`, `DELETE /admin/api/jobs/{id}` | Run `verify` (optionally against sources), `bench`, `download`, `build`, `update` as child processes with live logs; stop them |
| Reports — `GET /admin/api/reports`, `GET /admin/api/reports/{id}` | Build reports and benchmark results found in the data dir |
| Cache & tests — `GET/POST/DELETE /admin/api/cache`, `POST /admin/api/selftest {queries}` | Inspect, resize or clear the postings cache; run an in-process benchmark over freshly sampled queries |

## Migration from the legacy Go API

| Legacy | Now |
|--------|-----|
| `GET /api/v1/cnpj/{cnpj}` (14 digits) | Same path; also accepts alphanumeric CNPJ and 8-char roots |
| `GET /api/v1/estabelecimentos/{cnpj}` | Same path |
| `GET /api/v1/empresas/search?razao_social=` | Same path; trigram/FTS replaced by token + prefix matching |
| `GET /api/v1/estabelecimentos/search` (filters) | Same path; parameters as above |
| `POST /api/v1/export/csv`, `/export/phones` | `GET /api/v1/export` (`tem_telefone=true` for phone lists) |
| `GET /api/v1/stats/uf`, `/stats/cnae`, `/analytics/summary` | `GET /api/v1/stats?group_by=uf|cnae|...` |
| `GET /api/v1/lookup/*` (typeahead) | `GET /api/v1/dominios/{tabela}`, `GET /api/v1/search?q=` |
| API keys, quotas | Not part of the engine; use a gateway |
| Admin panel | `/admin` on the admin listener (metrics, operations, jobs) |
| `GET /health`, `/readyz`, `/metrics` | Same paths (+ `/healthz`) |

Response bodies changed: field names follow the RFB layout column names (as in
`FORMAT.md`) and every value is the original string.
