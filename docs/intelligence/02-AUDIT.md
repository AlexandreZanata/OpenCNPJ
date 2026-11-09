# 02 — Audit: what OpenCNPJ already gives the assistant

Audit of commit `8d30491` (branch `feat/accounts-quotas-desktop`, 2026-11-04). The engine
is reused as is; nothing in this plan requires changing the `.ocnpj` format or the
existing crates during the research phases (§8).

## 1. Summary

OpenCNPJ already answers, exactly and fast, almost every *data* question the assistant
needs on one snapshot: term filters (UF, município, CNAE at any level, situação, porte,
natureza, MEI/Simples, matriz/filial, motivo, CEP, DDD, contacts), date and capital
ranges, exact counts, `group_by` counts, yearly histograms, sorted lists and CNPJ lookups.
What it lacks for the assistant is (a) a language front end, (b) data that is not in the
RFB release (population, TOM ↔ IBGE codes, accented names, CNAE hierarchy labels,
geography), (c) history across monthly releases and (d) sub-10 ms national rankings on a
1 vCPU node, which calls for small precomputed aggregates.

## 2. Reusable components

| Component | Where | Reuse in the assistant |
|---|---|---|
| Accent/case folding, tokenization | `crates/opencnpj-core/src/text.rs` (`fold`, `tokenize`, `STOPWORDS`) | Basis of the question normalizer. The assistant needs its own variant: it must keep digits, `%`, `/`, `-`, `?` and must **not** drop `e`/`de`/`do` (they carry meaning: "Sorriso **e** Sinop", "**do** MT"). Same folding table so names normalize identically. |
| CNPJ ids and check digits | `crates/opencnpj-core/src/cnpj.rs` | Recognize and validate CNPJs (numeric and 2026 alphanumeric) typed in questions. |
| Snapshot reader | `crates/opencnpj-store/src/reader.rs` (`Snapshot::open`, `label`, `field_values`, `doc_values`, `numeric`, `postings`) | Source for the aggregates builder (WP-1.4) and for reference tables (municípios, CNAEs, naturezas, motivos are inside the snapshot, section 6). |
| Query model and execution | `crates/opencnpj-store/src/search.rs` (`Query`, `candidates`, `search`, `group_counts`) | Engine backend: exact counts (`SearchResult::total`), lists, `group_by`. Also the oracle for differential tests of the aggregates. |
| Doc values (município, main CNAE) | `crates/opencnpj-store/src/docvalues.rs` | One-pass counting per município/CNAE when building aggregates. |
| Numeric columns | `crates/opencnpj-store/src/numeric.rs` (`select`, `top_k`, `histogram`) | Opening/status dates, capital, MEI/Simples dates for flows and time series; "newest first" lists. |
| HTTP parameter grammar | `crates/opencnpj/src/params.rs` (`parse`) | **Lowering target of the IR**: an engine-backed IR compiles to the same `(key, value)` pairs the API accepts, so filter semantics have one implementation. |
| HTTP API | `docs/API.md`: `/api/v1/search`, `/stats`, `/cnpj/{id}`, `/export`, `/dominios`, `/snapshot`, `/campos` | Remote engine backend for a chat node that does not hold the snapshot. |
| Accounts, plans, quotas, response cache, coalescing, lanes | `crates/opencnpj/src/gate.rs`, `accounts.rs`, `api.rs` (`blocking`, `blocking_fast`) | Chat route behind the same gate with a new route kind (`kind_of` → `"chat"`). Response cache keyed by canonical IR instead of URL. |
| Metrics | `crates/opencnpj/src/metrics.rs` | Per-stage latency histograms (normalize, parse, plan, execute, render). |
| Privacy switches | `crates/opencnpj/src/output.rs` (`Privacy`), `--hide-socios`, `--hide-contacts` | Applied to every chat list. |
| Benchmark harness | `crates/opencnpj/src/bench.rs` (seeded PRNG, percentiles, JSON/CSV/MD reports, `--cold`) | Pattern and code to copy for the interpreter benchmark. |
| Load generator | `crates/opencnpj/src/loadtest.rs` | Chat load tests (users, think time, per-second timeline). |
| Test fixture | `crates/opencnpj-ingest/src/testkit.rs`, `crates/opencnpj/tests/common/mod.rs` | Small synthetic RFB release for end-to-end tests of the assistant. |
| Download of any competence | `crates/opencnpj/src/download.rs` (`--month YYYY-MM`) | Fetch past releases to build historical aggregates. |
| Deploy kit | `deploy/` (systemd unit, Caddy, `--reuse-port` upgrade) | Same pattern for the chat node. |
| `fst` with `levenshtein` feature | workspace dependency (`crates/opencnpj-store/Cargo.toml`) | Lexicon storage and bounded fuzzy matching, no new dependency. |

## 3. Data the engine can filter and group by

| Dimension | Index field (FORMAT.md) | Cardinality (2026-10) | Notes for the assistant |
|---|---|---|---|
| UF | 4 `uf` | 27 + `EX` | `EX` = abroad (173,410 establishments) |
| Município | 5 `municipio`, doc values | 5,572 codes in the table, 5,583 seen | RFB/TOM 4-digit codes; table has no UF; 241 homonym names |
| Main CNAE | 6 `cnae`, doc values | 1,359 subclasses in the table, 1,343 used | Prefix = division/group/class, letter = section |
| Secondary CNAE | 7 `cnae_secundaria` | multi-valued (70.5 M active pairs) | `group_by` takes 2–3 s on the engine (BENCHMARKS.md) |
| Situação | 8 `situacao` | 01 nula 110,547 · 02 ativa 28,380,653 · 03 suspensa 323,068 · 04 inapta 10,165,173 · 08 baixada 34,829,938 | [MEASURED], see results file |
| Porte | 9 `porte` (company level) | 00, 01, 03, 05 | Propagated to every establishment |
| Natureza jurídica | 10 `natureza` | 92 | Prefix = group (e.g. `2` = entidades empresariais) |
| Simples / MEI | 11, 12 (+ exclusion dates) | S/N | `mei_ativo` composite exists |
| Matriz/filial | 13 | 1/2 | |
| Motivo, CEP, DDD, contato | 17–20 | | CEP prefix enables neighbourhood-level filters later |
| Dates | numeric 1–6 | 1901-01-01 … 2026-10-11 | Opening, status, Simples/MEI opt-in and exclusion |
| Capital social | numeric 7 | | Company level, repeated on branches: never sum across establishments |

Total establishments 73,809,379; 2026 registrations up to 2026-10-11: 4,749,185 [MEASURED].

## 4. Engine latencies that drive the planner

From BENCHMARKS.md (format 1.2, national snapshot, 1 thread, warm, in process):

| Category | p50 / p95 ms | Consequence |
|---|---:|---|
| `lookup_cnpj` | 0.17 / 0.20 | Lookups always go to the engine |
| `filter_selective` (município + CNAE + ativa) | 4.4 / 9.2 | Lists in one city are cheap on the engine |
| `sort_newest_in_municipio` | 8.1 / 40.3 | "Empresas registradas recentemente em X" is fine on the engine |
| `filter_opened_in_year_uf` | 75 / 171 | National/UF date filters are slow for interactive chat → aggregates |
| `stats_municipio_by_uf` | 22.7 / 90.9 | Rankings of municípios within a UF: acceptable on a big node, too slow on 1 vCPU |
| `stats_uf_by_cnae` | 86 / 207 | National rankings → aggregates |
| `stats_cnae_secundaria_by_uf` | 2,232 / 3,362 | Never in the chat path → aggregates or refuse |

Rule derived from this: **counts, rankings, densities, breakdowns and time series come
from local aggregates; lists, lookups and rare filter combinations go to the engine**
([04-QUERY-IR.md](04-QUERY-IR.md) §7).

## 5. Gaps

| Gap | Why it matters | Resolution (WP in 14-ROADMAP.md) |
|---|---|---|
| G1 No population | "por habitante", "cidades com menos de 50 mil habitantes" | IBGE census 2022 + annual estimates (WP-1.2) |
| G2 No TOM ↔ IBGE mapping and no UF in the municipality table | Join population and geography; homonyms ("Bom Jesus" ×5) need the UF | Official TOM ↔ IBGE table; UF from it, data anomalies reported (WP-1.2) |
| G3 Names uppercase without accents ("CUIABA") | Display quality; matching accented input | IBGE official names for display; folded forms for matching (WP-1.2) |
| G4 CNAE table only has subclasses; no popular names | "padaria", "tecnologia", "construção" | CONCLA hierarchy + curated sector catalogue (WP-1.3) |
| G5 Only one competence (2026-10) on the host | "crescendo", "variação", "estavam ativas em 2025" | Download past releases, keep only their aggregates (WP-1.5) |
| G6 No geography | "cidades próximas" | IBGE municipal seats and mesh → neighbour lists and distances (WP-1.2) |
| G7 National `group_by` too slow for chat on 1 vCPU | Interactive rankings | Aggregates ("cube"), built from the snapshot (WP-1.4) |
| G8 No region concepts | "Centro-Oeste", "região de Sinop" | IBGE regions (grande região; regiões intermediárias/imediatas) (WP-1.2) |
| G9 Companies vs establishments | "quantas empresas" | Counted unit is explicit in the IR; head offices via `matriz_filial=1` |

## 6. Priority use cases mapped to data

| Use case (brief §9) | Example | Data path |
|---|---|---|
| Territorial intelligence | "Onde existe menor concentração de farmácias por habitante em Mato Grosso?" | Aggregates: active establishments by (município, CNAE set) ÷ IBGE population, ranked, min-base rule |
| Business evolution | "Quais setores ganharam mais estabelecimentos ativos nos últimos seis meses?" | Aggregates of two monthly snapshots (stock difference per sector); fallback registrations − baixas in the period, labelled |
| B2B search | "Quais empresas ativas de transporte existem em Sorriso?" | Engine: `/api/v1/search?municipio=9907&cnae=49,50,51,52,53&situacao=02` (lowered from the IR), paginated, privacy switches applied |

## 7. Measured aggregate sizes

See [results/2026-11-04-aggregate-cardinality.md](results/2026-11-04-aggregate-cardinality.md):
active stock by (município, CNAE, matriz/filial, porte, MEI) = 2.83 M cells; monthly
registrations since 2021 by (município, CNAE) = 8.7 M; monthly baixas since 2021 = 5.4 M;
secondary-CNAE (município, CNAE) active presence = 2.79 M. All fit a 1 GB node.

## 8. Integration constraints

* Research phases (roadmap phases 1–5) add **new crates and docs only**. They do not edit
  existing crates, the `.ocnpj` format or the public API.
* Aggregates are a separate derived artifact (`.ocagg`, [09-DATA-ASSETS.md](09-DATA-ASSETS.md)),
  versioned on their own, built from a verified snapshot. They never replace the snapshot
  as the source of truth.
* Product integration (phase 6) adds one route (`/api/v1/chat`), one route kind in
  `gate.rs`, and a serve flag; it starts only after the owner approves the decision record.
* Disk on the build host: `/` is full; use `/data` for `CARGO_TARGET_DIR`, datasets,
  aggregates and historical downloads.
