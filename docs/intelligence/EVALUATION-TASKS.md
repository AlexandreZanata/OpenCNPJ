# Evaluation tasks

Standing tasks that keep answers correct as the interpreter, catalogues and data change.
Each task has an id, a command, a cadence, a pass criterion and an evidence location, so a
reviewer can tell from the repository alone whether a change or a release was checked.

The guiding failure is the **silently wrong answer**: a question that executes and looks
plausible but ignores or inverts part of what was asked ("empresas agrícolas" answered with
every company of the city, "padarias fechadas" answered with active bakeries). Refusing or
asking is always preferable to that.

## Tasks

| Id | Task | Command | When | Pass | Evidence |
|---|---|---|---|---|---|
| ET-01 | Contract tests | `cargo test --workspace` (module `contracts::tests` and the dataset replays) | Every PR (CI) | All pass | CI log |
| ET-02 | Interpretation regression against `main` | `scripts/eval/run_tasks.sh <main bin> <candidate bin> <out>` | Every PR touching `opencnpj-nlu`, `opencnpj-query` or a catalogue | `worse = 0`; every `neutral` change explained in the PR | `regression.txt`, `changes.jsonl` |
| ET-03 | Interpretation contracts on every set | same script (`contracts` on all sets) | Every such PR | 0 findings on items scored correct; findings on wrong items opened as defects | `contracts/summary.json`, `findings.jsonl` |
| ET-04 | Data contracts on the snapshot | same script with `DATA_DIR` | Every snapshot release and before a deploy | 0 findings | `data-contracts/summary.json` |
| ET-05 | Backend differential | `opencnpj-intel diff-exec` / `diff-agg` (see `results/commercial-round-*/RUNBOOK.md`) | Every snapshot release | 0 mismatches | release results |
| ET-06 | Skill and probe scores | part of ET-02 | Every PR | skills-export, skills-company and probes at 100% | `regression.txt` |
| ET-07 | Live smoke on the service | `python3 docs/intelligence/results/skills-company-01/live_check.py http://<host>:5175` | After each deploy | Expected decisions, files open, counts match ET-04 | `live-check.txt` |
| ET-08 | Latency budget | `python3 docs/intelligence/results/skills-export-01/latency.py http://<host>:5175 web/src/features/chat/examples.ts` | Every release | Interpretation < 1 ms; sector-scoped answers < 50 ms; files of 1,000 rows < 2 s warm | `latency*.txt` |
| ET-09 | Probe intake | Add every newly found silent error to `eval/probes-NN` with its defect class | When a defect is found | The probe fails before the fix and passes after | probe set + PR |
| ET-10 | Human review sample | Review 50 answers from real usage against the records | Monthly, once usage exists | Findings filed as probes (ET-09) | review notes |

## Contracts

Checked by `opencnpj-intel contracts` (and, for CI-sized sets, by `contracts::tests`).

| Code | Contract | Level |
|---|---|---|
| `dropped_municipality`, `dropped_uf`, `dropped_region`, `dropped_sector` | A place or activity named in an executed question appears in its request (filter, exclusion or comparison member). Exempt: names folded by an exclusion, lowercase two-letter words that are not states there ("se", "para"), common words that are also cities ("passos"), CNPJ lookups | Interpretation |
| `executed_without_query`, `refused_with_query`, `query_and_company_search`, `file_for_meta_answer`, `empty_company_name`, `refusal_without_reason` | Decisions and payloads agree | Interpretation |
| `count_differs_from_records` | An aggregate count equals the records an equivalent list finds | Data |
| `record_outside_place`, `record_in_exclusion`, `record_outside_activity`, `record_in_excluded_activity`, `record_outside_status`, `record_outside_head_office`, `record_outside_opened`, `record_outside_status_date` | Every record of a list satisfies the query | Data |
| `file_rows_differ`, `file_error` | A file holds exactly the rows the answer promised | Data |
| `sentence_without_number` | The lead sentence states the number of the KPI tile | Data |
| `company_result_without_name` | An exact company search returns only names containing the searched words | Data |
| `execution_error` | An executed question runs | Data |

Catalogue contracts (CI): sector ids only grow at the end (`eval/contracts/sector-ids.txt`),
every sector CNAE is a subclass or a documented legacy subclass, and no word of the delivery
vocabulary is a place name except reviewed regional words.

## Defect classes found so far

| Class | Example | Fixed by | Probe family |
|---|---|---|---|
| Unknown descriptor dropped | "empresas agrícolas" → all companies | agro sector + fail-closed rule | `unknown_kind_dropped` |
| Status word inverted | "padarias fechadas" → active | status vocabulary | `closed_word_read_as_active`, `inactive_read_as_active` |
| Legal form read as status | "sociedade anônima fechada" → closed | masked before status words | `legal_form_read_as_status` |
| List period dropped | "padarias de 2025" → all active | clarification | `list_year_dropped` |
| Row count truncated | "Excel com 5000 padarias" → 20 | explicit counts kept | `row_count_truncated` |
| City before "do <estado>" dropped | "Belém do Pará" → national | state after "do" qualifies the city | `city_before_state_dropped` |
| Catalogue code that does not exist | `1012105` in meat processing | removed | catalogue contract |

## Log

Append one line per run: date, change or release, tasks run, outcome, evidence path.

| Date | Change | Tasks | Outcome | Evidence |
|---|---|---|---|---|
| 2026-11-09 | PR #8 (company search, agro, contracts) | ET-01–ET-04, ET-06, ET-07 | see `results/contracts-2026-11-09/REPORT.md` | `results/contracts-2026-11-09/` |
