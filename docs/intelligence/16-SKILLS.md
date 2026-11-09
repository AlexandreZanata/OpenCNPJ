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
| Assistant (interpret, execute, render, extras, files) | `crates/opencnpj-lab/src/assistant.rs` |
| HTTP service and `answer` CLI | `crates/opencnpj-lab/src/serve.rs` |
| Chat page | `web/src/pages/local-consulta-page.ts`, `web/src/features/chat/` |
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
* "no setor de TI", "no ramo de", "no segmento de" are accepted before a recognized activity.
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

## 8. Next skills

| Skill | Sketch |
|---|---|
| Summary ("resuma", "panorama de Sinop") | A fixed bundle of aggregate queries for one place: stock, top sectors, last 12 months, size distribution |
| Comparison of periods ("2025 contra 2024") | Two flow counts with the same scope and a difference block |
| Saved lists | Name an export and refresh it on the next snapshot |
| More formats | JSON and Parquet for analysts, through the same `Sheet` |
