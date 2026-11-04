# 01 — Requirements

What the OpenCNPJ conversational assistant ("the assistant") must do, for whom, and the
rules it must never break. Requirement ids (`FR-`, `SR-`, `CR-`, `NFR-`) are referenced by
the roadmap ([14-ROADMAP.md](14-ROADMAP.md)) and by the acceptance tests.

Status legend used across these documents: **[MEASURED]** (command and raw output exist),
**[ESTIMATE]** (computed from measured inputs, formula given), **[HYPOTHESIS]** (to be
validated by an experiment).

## 1. Purpose

Give managers and analysts **correct answers in well under a second** to questions in
Brazilian Portuguese about companies and establishments registered with Receita Federal,
without an LLM, a generative model or any paid/external AI service. The assistant
understands a closed domain with wide linguistic flexibility, turns each question into a
typed, validated query ([04-QUERY-IR.md](04-QUERY-IR.md)), runs it on deterministic data
structures and answers with templates that cite every number's source.

It is not a general chatbot. When it does not understand, or when the data cannot answer,
it says so or asks one short question.

## 2. Users and jobs to be done

| Persona | Typical question | What "good" means to them |
|---|---|---|
| P1 Public manager (municipal/state economic development) | "Quantas empresas de construção surgiram em MT este ano?" | Fast, comparable across municipalities, methodology they can quote in a report |
| P2 Business/expansion analyst | "Onde existe menor concentração de farmácias por habitante em Mato Grosso?" | Rankings and density with population source and year; exportable table |
| P3 B2B prospecting (sales) | "Quais empresas ativas de transporte existem em Sorriso?" | A filtered list of establishments with the allowed fields, paginated |
| P4 Journalist / researcher | "Quais setores ganharam mais estabelecimentos ativos nos últimos seis meses?" | Explicit metric definitions and data dates; no overstated claims |

## 3. Scope

**In scope (v1):** counts, lists, CNPJ lookup, comparisons, rankings, breakdowns, time
series of registrations/closures, change in active stock between monthly snapshots,
density per inhabitant, filters by place / sector / status / size (porte) / MEI / Simples /
head office vs branch / legal nature / dates / capital, short conversational follow-ups,
clarifying questions, methodology and data-date answers.

**Out of scope (v1):** free conversation; opinions or advice ("vale a pena abrir uma padaria
em Sinop?" is answered with data + an explicit caveat, never a recommendation); revenue,
profit, employees (not in the data); people search (owners, partners, CPF) through the
chat; predictions; any data not derived from RFB, IBGE or other sources listed in
[09-DATA-ASSETS.md](09-DATA-ASSETS.md).

## 4. Functional requirements

| Id | Requirement |
|---|---|
| FR-01 | Accept a question in pt-BR (formal, colloquial, without accents, with typos, abbreviations) and return a structured answer: text + data blocks (number, table, chart series) + methodology + sources + data date. |
| FR-02 | Count establishments (default) or head offices ("empresas sediadas") matching filters: place, sector, status, porte, MEI, Simples, matriz/filial, natureza jurídica, date ranges, capital range. |
| FR-03 | List establishments matching filters (B2B), with ordering (newest, oldest, capital) and pagination, returning only fields allowed by the deployment's privacy switches and the caller's plan. |
| FR-04 | Look up a CNPJ (14 or 8 characters, numeric or alphanumeric) given in the question. |
| FR-05 | Compare 2–4 places (municipalities, UFs, regions) or 2–4 sectors on one metric. |
| FR-06 | Rank places or sectors by a metric (absolute count, per 10,000 inhabitants, inhabitants per establishment, registrations, closures, change in active stock), ascending or descending, top-k, within a scope (e.g. "em MT"), with a minimum-base rule to avoid tiny-denominator artefacts. |
| FR-07 | Break a metric down by one dimension (porte, situação, matriz/filial, MEI, CNAE level, year). |
| FR-08 | Time series: registrations (data de início de atividade) and closures (baixas by data da situação cadastral) per month or year; change in active stock across available monthly snapshots. |
| FR-09 | Density: establishments per 10,000 inhabitants using IBGE population, always stating the population source and reference year. |
| FR-10 | Filter places by population ("cidades com menos de 50 mil habitantes") and by proximity ("cidades próximas de Sinop", by neighbours or distance), once the geographic assets exist. |
| FR-11 | Resolve popular activity names ("padaria", "oficina mecânica", "pet shop", "tecnologia") to a versioned set of CNAE codes, and always list the codes used in the answer. |
| FR-12 | Resolve place names with approximate matching and homonym handling ("Bom Jesus" exists in several UFs → ask which UF unless context disambiguates). |
| FR-13 | Multi-turn conversation: follow-ups add, replace or remove filters, compare with previous entities, correct the previous question, or start a new subject ([06-CONTEXT.md](06-CONTEXT.md)). |
| FR-14 | Ask one short clarifying question, with selectable options, when a required slot is missing or a term is ambiguous ("grandes", homonym city, unclear metric). |
| FR-15 | Refuse politely, with suggestions of supported questions, when the question is out of domain or asks for data that does not exist. |
| FR-16 | Explain itself on request: "como você calculou?", "de quando são os dados?", "quais CNAEs entraram?". |
| FR-17 | Return machine-readable output (the canonical query, the result rows) so the same answer can be exported or reproduced through the existing API. |
| FR-18 | Log unanswered and clarified questions (subject to the privacy switches) so the lexicon and grammar can be improved from real usage. |

## 5. Semantic rules (never broken)

These rules outrank coverage. An answer that breaks one is a defect even if the user is
happy with it.

| Id | Rule |
|---|---|
| SR-01 | **Registration is not physical opening.** `data_inicio_atividade` is the date of the cadastral start of activity. Answers say "registrados com início de atividade em …", never "abriram as portas". |
| SR-02 | **"Opened in X" ≠ "active in X" ≠ "active today".** "Quantas abriram em 2025" = registrations dated 2025 (any current status). "Quantas estavam ativas em 2025" = active stock *at a date in 2025*, answerable only from a snapshot of that time; otherwise the assistant says so and offers what it can answer (see SR-03). "Quantas estão ativas" = situação 02 in the served snapshot. |
| SR-03 | **No reconstructed history presented as fact.** Active stock at a past date can only be estimated from one snapshot (`opened ≤ date` and (`active now` or `status date > date`)); this ignores intermediate status changes. It is off by default; when enabled it is labelled "estimativa" with the method. |
| SR-04 | **Closures are status events.** "Fecharam" = situação 08 (BAIXADA) with `data_situacao_cadastral` in the period. The status date is the date of the *last* status change. Suspensa (03) and inapta (04) are not closures; when asked "quantas fecharam", the answer is baixadas and mentions inaptas separately if relevant. |
| SR-05 | **Growth needs a definition.** "Crescendo" defaults to *change in active establishments between two monthly snapshots*; if fewer than two snapshots cover the period, it falls back to *registrations in the period* and the answer names the metric used. Percent change uses a minimum base (default 30 establishments) to avoid "+400 % (1 → 5)". |
| SR-06 | **Establishment vs company.** The default unit is the establishment (one CNPJ of 14 characters). "Empresas" in a question is read as establishments unless the user asks for companies; answers state the unit. "Empresas sediadas em X" = head offices (matriz) in X. |
| SR-07 | **Sector = explicit CNAE set.** A sector name maps to a versioned list of CNAE subclasses (main CNAE by default; secondary only when asked). The list is shown. A sector with no curated mapping is resolved from CNAE descriptions only when unambiguous, otherwise the assistant asks. |
| SR-08 | **Porte is RFB porte.** "Micro" = porte 01, "pequeno porte" = 03, "demais" = 05, "não informado" = 00. "Grande" has no RFB code: the assistant asks whether the user means porte "demais" or a capital threshold. MEI is a separate flag (Simples table), and MEI establishments normally also have porte 01. |
| SR-09 | **Capital social is not revenue**, density is not opportunity, a low competitor count is not proof of demand. Answers that rank by these add the matching caveat from the caveat catalogue ([08-RESPONSES.md](08-RESPONSES.md)). |
| SR-10 | **Time words are anchored to the data, not to the clock.** "Últimos seis meses", "este ano", "ano passado" are resolved against the served snapshot's reference date and the resolved dates are printed. |
| SR-11 | **Every number comes from a query.** No number in an answer is produced by a template, a classifier or a rule; templates only format values returned by the executor. |
| SR-12 | **Same meaning → same canonical query → same answer.** Paraphrases must normalize to equal canonical IR (and hit the same cache entry); different meanings must not. |
| SR-13 | **Negation is honoured or refused.** "Que não sejam MEI", "sem filiais", "exceto Cuiabá" become exclusion filters; a negation the grammar cannot attach to a slot causes a clarification, never silent dropping. |
| SR-14 | **Personal data stays behind existing policies.** The chat never searches people by name or CPF, never shows partners, and applies `--hide-socios` / `--hide-contacts` and plan rules to every list. Owner CPFs embedded in MEI/EI company names are masked in chat output by default (display only; decision D-05). |

## 6. Conversational requirements

| Id | Requirement |
|---|---|
| CR-01 | Context is a compact structure (target ≤ 256 bytes encoded), not the text history. |
| CR-02 | Follow-up operations are distinguished: add filter, replace filter, remove filter, compare with previous entities, refer to previous results ("dessas cidades"), correct the last question, answer a clarification, start a new subject. |
| CR-03 | A new, self-contained question starts a new subject: filters are not inherited unless the utterance is elliptical or anaphoric. |
| CR-04 | After a clarifying question, the next utterance is first interpreted as its answer (option number, option words, or a free value), and only then as a new question. |
| CR-05 | The assistant shows how it understood the question (one line: "Entendi: estabelecimentos ativos · Padaria (CNAE 4721-1/01, 4721-1/02) · Sorriso/MT") so users can catch misreadings. |

## 7. Non-functional requirements

Targets are experimental: the benchmark protocol ([12-BENCHMARK-PROTOCOL.md](12-BENCHMARK-PROTOCOL.md))
decides whether they are met; a missed target is documented with the reason and the cost
of alternatives, never hidden.

| Id | Requirement | Target |
|---|---|---|
| NFR-01 | Interpretation latency (text → validated canonical IR), 1 vCPU | p95 < 10 ms (required by the brief); design goal p95 < 1 ms **[HYPOTHESIS]** |
| NFR-02 | Aggregate answer latency (IR → result from the local aggregates), 1 vCPU | p95 < 10 ms **[HYPOTHESIS]** |
| NFR-03 | End-to-end chat latency for aggregate-backed answers, server side | p95 < 50 ms; lists and lookups add the engine's own latency (BENCHMARKS.md) |
| NFR-04 | Additional resident memory of the interpreter (lexicon, grammar, optional classifier) | < 20 MB |
| NFR-05 | Optional classifier artifact size | < 5 MB (design goal < 1 MB) |
| NFR-06 | Chat node footprint | runs on 1 vCPU / 1 GB RAM / no GPU with the aggregates of ≥ 12 monthly snapshots, ≥ 300 connected clients, explicit concurrency limits |
| NFR-07 | Accepted-answer precision (accepted queries whose canonical IR equals the gold IR) on the blind human set | ≥ 99 % (design goal 99.5 %) |
| NFR-08 | Coverage (in-domain questions answered without clarification) on the blind human set | ≥ 85 % for v1, tracked per intent |
| NFR-09 | Out-of-domain rejection rate on the OOD set | ≥ 95 % |
| NFR-10 | Dependencies | No LLM, no generative model, no external/paid AI API, no Python/PyTorch at runtime, no PostgreSQL/Redis/Elasticsearch, no vector database; new Rust crates only with a stated reason |
| NFR-11 | Reproducibility | Every artifact (lexicon, grammar, aggregates, classifier) is rebuilt bit-for-bit from versioned inputs with recorded seeds; each has a SHA-256 and a version; rollback = switch to the previous artifact |
| NFR-12 | Correctness of aggregates | Differential test: every aggregate-backed answer equals the engine's exact count on the same snapshot (sampled, plus the full golden set) |
| NFR-13 | Maintainability | Adding a sector, synonym or city alias is a data-file change with tests, no code change; adding an intent touches the grammar file, the IR enum and the templates only |
| NFR-14 | Repository rules | English code and docs; no national datasets or large binaries in Git; snapshot layout changes go through FORMAT.md and the format version (AGENTS.md) |

## 8. Hardware baseline [MEASURED, 2026-11-04]

| Machine | Facts | Use in this plan |
|---|---|---|
| Development / build host | Intel i7-13620H (6 P-cores with HT at up to 4.7–4.9 GHz = CPUs 0–11, 4 E-cores at up to 3.6 GHz = CPUs 12–15), 31 GiB RAM, NVMe. `/` has **3.6 GB free (100 % used)**; `/data` (NVMe, 916 GB) has 109 GB free. Rust 1.99.0, DuckDB CLI available, Python 3.12 without duckdb/pyarrow. | All builds, datasets and experiments. **Every large output goes to `/data`** (e.g. `CARGO_TARGET_DIR=/data/opencnpj-target`, datasets under `/data/opencnpj-data/intelligence/`). |
| National data on that host | `/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj` (8.33 GB, served as `current`), raw ZIPs of 2026-10 (7.3 GB), Parquet baselines (4.9 GB). Only **one competence (2026-10)** is present. | Ground truth for aggregates and differential tests. History needs more competences (WP-1.5). |
| Production target for the chat node | 1 vCPU, 1 GB RAM, no GPU (from the brief). | Emulated locally: one E-core (`taskset -c 12`) inside a 1 GB memory cgroup (12-BENCHMARK-PROTOCOL.md §6). |
| Production data node (full engine) | 8–16 cores, 16 GB RAM, NVMe (DEPLOYMENT.md). | Serves lists, lookups and non-aggregated queries through the existing API. |

## 9. Definition of done for v1 ("functional and professional")

1. All NFR targets met or explicitly waived in [15-DECISIONS.md](15-DECISIONS.md) with measurements.
2. The three priority use cases (territorial intelligence, business evolution, B2B search;
   [02-AUDIT.md](02-AUDIT.md) §6) work end to end on the national data, through the API and
   a minimal web chat page, with methodology and data date in every answer.
3. The blind human test set, the OOD set and the dialogue scripts pass at the agreed thresholds.
4. Monthly update rebuilds every artifact automatically and can roll back.
5. Delivery gate passes (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`).

## 10. Decisions needed from the project owner

Each has a recommendation; the roadmap proceeds with the recommendation unless told otherwise.

| Id | Question | Recommendation |
|---|---|---|
| D-01 | Default unit when users say "empresas" | Establishments, stated in every answer, with a one-click "contar só matrizes" |
| D-02 | Default status | Active (situação 02) for stock questions; any status for registration counts |
| D-03 | Main vs secondary CNAE for sectors | Main CNAE by default; "incluindo atividade secundária" switches to either |
| D-04 | How many past monthly snapshots to keep as aggregates | 24 months (aggregates only; full snapshots are not needed) |
| D-05 | Mask CPFs embedded in MEI/EI names in chat lists | Yes, display-only masking (`***.456.789-**` style), configurable |
| D-06 | Chat available to which plans | Same accounts and plans as the API, with a separate `chat` route kind and quota |
| D-07 | Store conversation text | No; store only canonical IR, outcome and timing; raw text only for unanswered questions, under `search_log` retention |
| D-08 | Question without a place ("quantas padarias existem?") | Answer for Brazil with a note offering a UF or city |
| D-09 | Single-snapshot estimate of past active stock (SR-03) | Off; enable later only if users ask and the label is accepted |
