# 16 — Delivery skills: files, chat answers and follow-ups

A *skill* is a deterministic capability layered on top of the data question. The first
skill is **export**: "me mande um Excel com todas as empresas que tiveram baixa em Mato
Grosso no setor de TI" returns the answer in the chat **and** a file (Excel, CSV or TXT)
with the matching establishments, restricted to the columns the user named ("apenas o
nome e o número"). No model generates text or guesses: the same question, conversation
and data always produce the same answer and the same file.

## 1. Goals and non-goals

| Goal | How |
|---|---|
| Ask for a file in plain Portuguese | Format, request and column phrases are recognized by the NLU skill layer |
| Choose columns | Typed column vocabulary (`ExportField`, 22 RFB columns) |
| Chat-like answers | The existing renderer (`08-RESPONSES.md`) wired to a service: lead sentence, KPI, charts, tables, records, provenance, follow-ups |
| Follow-ups ("agora em CSV", "só o nome e o CNPJ") | The export is part of the conversation state (`06-CONTEXT.md`) |
| Speed | One long-lived process keeps every asset in memory; no per-question process or reload |
| Correctness | Every row comes from the snapshot through the same filters the chat answer uses; nothing is sampled or estimated |

Out of scope: partners (sócios) and CPF searches (still rejected, SR-14), revenue or any
metric absent from the RFB data (still rejected), scheduled or e-mailed delivery.

## 2. Architecture

```
question ──► skills::delivery ──► rest of the question ──► grammar::resolve ──► Request (IR)
               │ format, columns, notes                         │ export=true: a count of companies
               ▼                                                ▼ becomes the list of those companies
            Export { format, fields, rows }            validate ─► plan ─► aggregates | snapshot
                                                                        │
            Interpretation { request, export } ◄────────────────────────┘
                     │
                     ├─► render (text, kpi, chart, table, records, provenance)
                     ├─► extras (monthly series, sample of real establishments)
                     └─► export offer ──► POST /api/local/export ──► xlsx | csv | txt
```

| Piece | Location |
|---|---|
| `Export`, `ExportFormat`, `ExportField`, default columns | `crates/opencnpj-query/src/export.rs` |
| Delivery phrase extraction | `crates/opencnpj-nlu/src/skills.rs` |
| Export-aware grammar (record lists, event filters, `no setor de`) | `crates/opencnpj-nlu/src/grammar.rs` |
| Conversation memory of the export | `Conversation::export` in `crates/opencnpj-nlu/src/context.rs` |
| Writers (XLSX, CSV, TXT) and record cells | `crates/opencnpj-insights/src/export.rs` |
| Value tables and titles for files | `render::value_table`, `render::title` |
| Assistant (interpret, execute, render, extras, files) | `crates/opencnpj/src/assistant/` (`mod.rs`, `assets.rs`, `engine.rs`, `geo.rs`) |
| Product routes (`/api/v1/assistant/*`, accounts and quotas) | `crates/opencnpj/src/assistant/http.rs`, see `17-PROSPECTING.md` §5 |
| Local development service and `answer` CLI | `crates/opencnpj-lab/src/serve.rs` |
| Chat page | `web/src/pages/assistant-page.ts`, `web/src/features/chat/` |
| Evaluation set | `crates/opencnpj-lab/eval/skills-export-01/` |

### 2.1 Why the export is not in the query IR

The export is a delivery instruction, not a query: the same canonical `Request` answers
the chat and fills the file. Keeping it beside the request (in `Interpretation.export`)
leaves the IR wire format (`IR_VERSION` 1), cache keys, golden fixtures and renderer
seeds unchanged, so every existing evaluation round scores exactly as before.

## 3. The NLU skill (`skills.rs`)

The delivery phrase is cut from the question **before** the grammar runs, so its words
can never act as a place, sector, filter or intent ("em um Excel" is not a place,
"com nome e telefone" is not a contact filter). The rest keeps its original spelling
and case, which the grammar needs for UF acronyms.

| Element | Examples | Result |
|---|---|---|
| Format | Excel, exel, planilha, xlsx · CSV · TXT, arquivo de texto, bloco de notas | `ExportFormat` |
| Export verb without format | exporte, exportar, download | Excel, with a note |
| Request words | me mande, envie, gere, quero, pode me mandar, por favor | removed |
| Column list | apenas/somente/só …, com …, contendo …, com as colunas … | `fields` in the user's order |
| Bare "número" | "apenas o nome e o número" | CNPJ, with a note (phone is "telefone") |
| "todos os dados" | todas as colunas, dados completos | all 22 columns |

The vocabulary is data: `crates/opencnpj-nlu/lang/pt-BR/delivery.toml`. Every list may
include another with `@name` ("pedido" = courtesy + pronouns + modal + verbs + regional
tags), resolved recursively when the interpreter starts; a cycle or an unknown name is an
error. Regional lists cover the South, Northeast, Minas, Centre-West, North and
Southeast, and are cut only around a file request ("Bah, tchê, me passa uma planilha…",
"Oxe, gera um CSV aí…, visse?"). A file noun with a hand-over verb ("me manda uma
tabela", "puxa um relatório") asks for Excel; an unsupported format (PDF, Word, JSON)
gets a clarification.

A column list after `com` needs two names unless it follows the format directly
("um Excel com o CNPJ das padarias"), so `com capital social acima de…` stays a filter.

### Grammar changes

* `resolve(…, export)`: with a file requested, a count of companies (stock, closures,
  registrations) becomes `Op::List`; rankings, comparisons, series and distributions keep
  their operation and export their table.
* Lists no longer drop the event they name: "padarias que fecharam em 2025" lists
  status 08 with `status_date` in 2025; "abertas em 2025" sets `opened` and every status.
  Without a period, "todas as empresas que tiveram baixa" lists every closed company.
* Repeated qualifiers such as "na cidade, no município de Sorriso" and "no ramo, no
  setor de farmácias" resolve to one filter. Unknown activities and places still require
  clarification.
* Record commands allow regional and courteous wording, repeated verbs and limits such as
  "me passa só 5 farmácias". Restating the same contact condition keeps the filter.
* Repeated aliases for one file format, such as "um CSV, em formato CSV", produce one
  export. The [redundant phrasing fixture](../../crates/opencnpj-lab/tests/fixtures/redundant-phrases.tsv)
  checks the operation, scope, contact filter, limit and format against the production
  catalogue. The [regional phrasing fixture](../../crates/opencnpj-lab/tests/fixtures/regional-phrases.tsv)
  covers all 27 UFs, municipality sets, whole-state queries, stock status, regional vocabulary
  and scoped contact expressions. When a delivery phrase names two different supported
  formats, the assistant asks the user to choose one. Run the fixtures with
  `cargo test -p opencnpj-lab search_phrases_keep_the_expected_activity_and_intent`.
* "baixados em" and "encerrados em" join the closure cues (masculine nouns).

### Conversation

`Conversation::export` holds the file of the thread. Refinements keep it ("e só as
matrizes?"), a new question clears it. A delivery whose remaining words only point at the
previous answer ("agora me mande isso em Excel", "e em CSV") reuses the previous request:
a count of closures becomes the list of those closures over the same period. A bare column
list ("só o nome e o CNPJ") changes the columns, and a new format keeps them.

## 4. Files

| Format | Dialect |
|---|---|
| XLSX | Two sheets: **Dados** (bold header, frozen first row, filters, widths, numbers and dates as typed cells) and **Sobre** (question, IR, source, competence, snapshot SHA-256, row count, privacy note) |
| CSV | RFB dialect: `;`, CRLF, UTF-8 with BOM (opens as columns in a Brazilian Excel), dates `dd/mm/aaaa`, decimal comma |
| TXT | Report: title, facts, then an aligned table (numbers right-aligned) |

Rules:

* The server re-canonicalizes and re-validates the request it receives; a client cannot
  widen it. Rows are capped by `--export-max-rows` (default 100,000) and the reply says
  when a file holds fewer rows than the total.
* Owner CPFs embedded in MEI/EI names are masked (SR-14, D-05). `--hide-contacts` removes
  phone and e-mail columns and the reply says so.
* Records are decoded in document order and returned in the list order (CNPJ, newest,
  oldest, capital).

## 5. Chat answers

`POST /api/local/ask` with `{question, context}` returns:

```json
{
  "interpretation": { "decision": "execute", "act": "new_query", "notes": [], "export": { "format": "xlsx", "fields": [], "rows": null } },
  "answer": { "decision": "execute", "understood": "…", "blocks": [ { "type": "text", "text": "…" }, { "type": "records", "columns": ["CNPJ", "…"], "rows": [["…"]], "total": 0, "page": 1 } ], "suggestions": ["Agora em CSV", "Só o nome e o CNPJ"] },
  "extras": [ { "type": "series", "title": "Evolução mensal de 10/2025 a 09/2026", "series": [ { "name": "Aberturas", "points": [] } ] } ],
  "export": { "format": "xlsx", "filename": "opencnpj-….xlsx", "rows": 0, "total": 0, "columns": ["CNPJ", "…"], "notes": [], "request": {}, "export": {} },
  "context": { "last": {}, "focus": [], "pending": [], "result_members": [], "export": null },
  "timing_us": { "interpret": 0, "execute": 0, "render": 0, "extras": 0, "total": 0 }
}
```

`POST /api/local/export` with `{request, export, question}` returns the file. The page
renders the blocks as one assistant turn: the lead sentence with emphasized numbers, KPI
tiles, the file card, charts (line with crosshair and table view; horizontal bars),
tables, the establishments with CNPJ and cadastral columns, "Como cheguei a esse
resultado" (methodology, caveats, source) and follow-up chips. Count answers also show
the monthly openings and closures of the last twelve complete months (aggregates only)
and the newest establishments behind the number (snapshot only).

## 6. Running

```sh
cargo build --release -p opencnpj-lab --bin opencnpj-intel
sh web/scripts/build.sh
./target/release/opencnpj-intel serve \
  --agg "$DATA/intelligence/agg/2026-10.ocagg" \
  --history "$DATA/intelligence/backfill/aggregates" \
  --snapshot "$DATA/snapshots/2026-10-balanced-1.2.ocnpj" \
  --web web/dist --listen 127.0.0.1:5174
```

`--listen` repeats (loopback and a LAN address in one process). A LAN address needs
`--allow-network <subnet>`; other clients get 403. Without
`--snapshot` the service answers aggregate questions and declines lists and files of
companies. One turn and its file from the command line:

```sh
./target/release/opencnpj-intel answer --agg … --snapshot … \
  --text "me mande apenas o nome e o numero em um exel com todas as empresas que tiveram baixa em mato grosso no setor de TI" \
  --export-dir /tmp/opencnpj-files
```

## 7. Evaluation loop

`crates/opencnpj-lab/eval/skills-export-01` is generated by `author_skills_export.py`
from templates, with gold IR **and gold export** built independently of the interpreter:
format prefixes and suffixes, column lists (the owner's "apenas o nome e o número"
shape included), closures with and without a period, registrations, active stock,
analytical exports, four-turn dialogues (count → "agora me mande isso em Excel" → "só o
nome e o CNPJ" → "agora em CSV"), and questions that must be declined or clarified.
`eval-questions` scores the export spec together with the IR (`Gold::export`).

The loop for the next skills round follows the commercial rounds: write new surfaces
(ideally from real user questions), run `eval-questions --interpret-only`, fix the
general cause in the skill or grammar, add a regression test, and re-run every earlier
round to confirm no prediction changed. Results of this round, with commands and raw
output, are in [results/skills-export-01](results/skills-export-01/REPORT.md).

## 8. Company search

Questions like "encontre a empresa zaflas", "zaflas", "dados da empresa X" or
"qual o CNPJ da Padaria Pão Quente em Cuiabá/MT?" look up companies by name (razão social
or nome fantasia) directly against the snapshot index, instead of rejecting with
"Não identifiquei uma pesquisa cadastral".

### 8.1 Flow and NLU

```
question ──► delivery.company_name(text, bare) ──► place_suffix (strip municipality/UF)
               │                                       │
               ├─► if 8+ digits ──► Op::Lookup         ├─► if only sectors ──► Op::List
               ├─► privacy/unavailable ──► Reject      └─► Interpretation.company = Some(CompanySearch)
               ▼
   assistant.ask() ──► opencnpj_store::search::search (exact: fuzzy=false)
                         │ total == 0? ──► retry with fuzzy=true
                         ▼
        ┌────────────────┴────────────────┐
        ▼ single root                     ▼ multiple roots
 company block + full fields        summary sentence + records table (preview)
 + records table if >1 branch       + up to 5 "Dados do CNPJ <cnpj>" suggestions
```

1. **Extraction**: `Vocabulary::company_name(text, bare)` checks for search openers
   (`delivery.toml` lists `busca` and `empresa`) or bare short names (`end - start <= 5`).
   Interrogative words, dialogue connectives, request prefixes, and conversational OOD words
   are rejected.
2. **Disambiguation**:
   - Numeric tokens with 8 or more digits route to CNPJ lookup rules (`Op::Lookup`).
   - Pure sector names ("busque padarias em Cuiabá") route to cadastral record lists (`Op::List`).
   - Mixed sector + cadastral filters ("padarias com telefone", "farmácias abertas") remain cadastral queries.
   - Privacy cues (`dono`, `cpf`, `socio`) and unavailable metrics (`faturamento`) remain refusals.
3. **Execution (`assistant.rs`)**:
   - Queries the snapshot via `opencnpj_store::search::search` with `q`, `field: "any"`, `fuzzy: false`,
     bounded by TOM municipality codes and UF codes if extracted by `place_suffix`.
   - If total is 0, retries with `fuzzy: true` and appends a note:
     *"Não achei esse nome exato; mostrei nomes parecidos."*
   - Single company root: returns a `"company"` block with all cadastral fields (respecting `--hide-contacts`),
     a lead sentence (`"Encontrei <razão> (<CNPJ>), situação X, em <município>/<UF>."`), and a `"records"`
     table if the company has multiple establishments.
   - Multiple company roots: returns a summary sentence with total count, a `"records"` preview table,
     and up to 5 `"Dados do CNPJ <cnpj>"` suggestions that route directly to `Op::Lookup`.
   - Zero results: returns a polite message:
     *"Não encontrei empresas com o nome “X”. Confira a grafia ou informe o CNPJ."*
4. **Extras for Lookup**:
   - When executing `Op::Lookup`, `assistant.extras()` queries the snapshot for other establishments
     sharing the same 8-digit root prefix (`Query.cnpj_prefixes`) and attaches them as an extra
     `"records"` table titled *"Estabelecimentos desta empresa"*.

Results: [results/skills-company-01](results/skills-company-01/REPORT.md).

## 9. Broad activities and unknown descriptors

"Lista de empresas agrícolas em Sorriso/MT" once listed every active establishment of the
city (banks included): the catalogue had only specific farm sectors (soja, milho, gado…),
and an unrecognized word describing the companies was dropped. Two changes:

* The sector `agriculture_livestock` (appended, so earlier sector ids are unchanged) holds
  every subclass of CNAE division 01 under the words users say: agrícola(s), agricultura,
  agropecuária, fazenda(s), lavoura(s), produtor(es) rural(is), pecuária, agricultor,
  fazendeiro. "Setor do agro" also uses division 01.
  Longer aliases still win ("fazendas de soja" stays soybean). "Agronegócio" and
  "empresas do agro" still ask for an activity because the supply chain spans multiple divisions.
* Fail closed: a word right after a plural "empresas / estabelecimentos / negócios / firmas
  / companhias" that is no catalogue entity and no known status, size or connective
  ("empresas xyzolicas") gets "Qual atividade você quer pesquisar?" instead of answering
  every company. The singular "a empresa Zaflas" names one company and is left to §8.

The sector catalogue also covers common service and trade phrases such as "escritórios
contábeis", "oficinas de lanternagem", "delivery de marmitas" and "provedores de banda
larga". A complete phrase takes precedence over shorter matches inside it. Broad terms
such as "comércio de vestuário" still need a narrower activity, for example "varejo de
vestuário" or "comércio atacadista de vestuário".

Search commands such as "procure" and "mostre" list records when they are followed by
a recognized activity and filters. Named companies still use the company search. A
phrase such as "empresas de produção de soja" identifies an activity, while a request
for the volume of soybean production needs a different source. The
[sector phrase fixture](../../crates/opencnpj-lab/tests/fixtures/sector-phrases.tsv)
checks the expected CNAEs, municipality, operation, contact filter and export format.

Record searches reuse the courtesy, regional and delivery phrases used for files.
A recognized activity can follow qualifiers such as "na área de" or "do ramo de".
Contact clauses such as "que têm um número de telefone" and "com endereço de e-mail"
apply to both lists and exact counts of establishments in the served snapshot. An
explicit count remains a count. Missing-contact and either-contact conditions prompt
for clarification, while "telefone e e-mail" requires both fields.

## 10. Repeated lines in a file

Rows are establishments. When the chosen columns leave out the CNPJ ("apenas o nome e o
telefone"), the branches of one company look like repeated lines. They are never removed
(that would drop establishments); the answer and the "Sobre" sheet say how many lines
repeat and suggest including the CNPJ or asking "só as matrizes" for one line per company.

## 11. Next skills

| Skill | Sketch |
|---|---|
| Summary ("resuma", "panorama de Sinop") | A fixed bundle of aggregate queries for one place: stock, top sectors, last 12 months, size distribution |
| Comparison of periods ("2025 contra 2024") | Two flow counts with the same scope and a difference block |
| Saved lists | Name an export and refresh it on the next snapshot |
| More formats | JSON and Parquet for analysts, through the same `Sheet` |

