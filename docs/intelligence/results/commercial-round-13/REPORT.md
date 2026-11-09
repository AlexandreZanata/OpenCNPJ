# Commercial Evaluation Round 13 — Final Report

Review date: 2026-11-08. Data anchor: `2026-10-11`; competence `2026-10`.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable completion index: [index.json](index.json).
Dataset card: [DATASET-CARD.md](../../../../crates/opencnpj-lab/eval/commercial-round-13/DATASET-CARD.md).

## 1. Executive Summary

- **Dataset size and batch structure**: **50,000** pt-BR commercial questions/turns across **99 batches** (`batch-001.jsonl` pilot with **1,000** items, then 98 batches of **500**).
- **Independent gold-label audit (`dataset-audit.json`)**: **50,000 / 50,000** passed deterministic checks; **0** normalized overlap against the **310,000** archived texts of rounds 01–12 and the frozen suites; **0** internal duplicates; **0** cross-split scenario/skeleton/session/CNPJ-root leakage; **142 / 142** MT municipalities.
- **Pilot**: `974 / 1,000` before fixes; **`1,000 / 1,000`** after fixes (`decision_correct = 1,000`, `gold_backend_ok = 856/856`).
- **National (`national/summary.json`)**:
  - `items`: **50,000**
  - `interpretation_correct`: **50,000 / 50,000 (100.00%)**
  - `decision_correct`: **50,000 / 50,000 (100.00%)**
  - `gold_backend_ok`: **42,800 / 42,800 (100.00%)**
  - `correct_executed_answer`: **42,800 / 42,800 (100.00%)**
  - `act_correct`: **40,139 / 41,800 (96.03%)**
  - `execution_fresh = 41,098`; `execution_reused_identical_ir = 1,702`
  - Decisions: `execute = 42,800`, `clarify = 4,400`, `reject = 2,800`
- **Differential verification**: `engine_match = 27,000`; `single_backend = 12,600`; `no_independent_reference = 1,993`; `marginal_match = 207`; `not_numeric = 1,000`; **`mismatch = 0`**.
- **Cargo gate**: fmt, clippy `-D warnings`, test (**48** NLU tests + workspace, 0 failures) and release build all exit `0`.

## 2. Quotas, Coverage and Provenance

| Provenance class | Items | Correct |
|---|---:|---:|
| `synthetic_regional_grounded` | 30,000 | 30,000 |
| `synthetic_business_scenario` | 20,000 | 20,000 |
| `public_observed_adapted` | 0 | 0 |
| **Total** | **50,000** | **50,000** |

| Primary family | Quota | Authored | Raw correct | Backend OK |
|---|---:|---:|---:|---:|
| `market_size_density_competition` | 5,000 | 5,000 | 5,000 | 4,000 |
| `prospecting_qualification_territory` | 7,000 | 7,000 | 7,000 | 7,000 |
| `historical_stock_flows_seasonality` | 5,000 | 5,000 | 5,000 | 5,000 |
| `geography_catchments_comparisons` | 5,000 | 5,000 | 5,000 | 5,000 |
| `corporate_structure_hq_branches_identity` | 4,000 | 4,000 | 4,000 | 4,000 |
| `supply_chain_complementary` | 5,000 | 5,000 | 5,000 | 5,000 |
| `cross_variable_correlation` | 5,000 | 5,000 | 5,000 | 4,000 |
| `dialogue_corrections_references_planning` | 6,000 | 6,000 | 6,000 | 5,800 |
| `source_quality_provenance_uncertainty` | 3,000 | 3,000 | 3,000 | 1,000 |
| `unsupported_missing_contradictions` | 3,000 | 3,000 | 3,000 | 0 |
| `tax_size_legal_capital_numeric` | 2,000 | 2,000 | 2,000 | 2,000 |
| **Total** | **50,000** | **50,000** | **50,000** | **42,800** |

- **Geography**: MT `30,000` (six strata); other 26 UFs `12,000` (per-UF **405–520**, all `>= 400`); national/cross-UF `8,000`.
- **Municipalities**: **3,174** distinct; **142 / 142** MT.
- **Registers**: `conversational_professional` 35,000; `formal_technical` 10,000; `noisy_short` 5,000.
- **Splits**: dev 35,000; eval 10,000; challenge 5,000.
- **Floors**: MT-context **34,190**; other-UF regional **5,822**; novel constructions **22,181**; clarify/reject **7,200**.

## 3. General Interpreter Corrections

1. **More sector colloquialisms**: `autocenter`/`autocentro`, `troca de óleo`, `vulcanizadora`, `atacarejo` and `charutaria` mapped to existing sectors.
2. **Plural exclusion coordination**: `exceto nas cidades de A e B` now accepts both entities; the exclusion pre-check accepts plural function words.
3. **Two-municipality exclusion**: new geography subvariant, with a single-exclusion fallback for states with one catalogued municipality (DF), avoiding a mislabeled cross-UF municipality.
4. **Capital ceiling edits**: `capital social de até X` parses as an upper bound; a compound keep-MEI/drop-capital/switch turn clears the ceiling while preserving MEI (`compound_edit`).
5. **Capital ceiling filters**: prospecting MEI list and tax head-office count under an upper capital bound.
6. **Exact day-level windows**: registration flows over explicit `DD/MM/YYYY` ranges.
7. **CNAE-section concentration**: cross-variable breakdown by CNAE section.
8. **Sector-name collision guard**: three-city comparisons and two-municipality exclusions avoid municipality names that are also sector surfaces (`Serraria`).

## 4. Metamorphic Consistency

`metamorphic-checks.json`: **PASS**, **0 violations** — 1,130 equivalence groups (2,832 items), 1,468 monotonic pairs, 1,900 partition sums, 1,990 exclusion bounds, 1,677 ranking prefixes.

## 5. Frozen Suites and Replays

`H-blind` was not opened or executed. Rounds 01–12 remain byte-identical.

| Frozen suite | Items | Round 12 | Round 13 | Decision correct |
|---|---:|---:|---:|---:|
| `adv.jsonl` | 304 | 214 / 304 | **214 / 304** | 285 / 304 |
| `h-informal.jsonl` | 202 | 121 / 202 | **121 / 202** | 172 / 202 |
| `dialogues.jsonl` | 122 | 84 / 122 | **84 / 122** | 106 / 122 |
| `ood.jsonl` | 400 | 400 / 400 | **400 / 400** | 400 / 400 |

Prior replays: `01` 1,000/1,000; `02` 950/1,000; `03` 2,844/3,000; `04` 4,826/5,000; `05` 9,654/10,000; `06` 19,312/20,000; `07` 19,312/20,000; `08` 48,259/50,000; `09` 49,975/50,000; `10` 50,000/50,000; `11` 50,000/50,000; `12` **50,000/50,000**.

## 6. Limitations and Next Adjustments

1. Sector catalogue still serialized as version `1`; aliases were added without breaking prior datasets. Future release should bump to v2 with migration.
2. `act_correct = 40,139 / 41,800`; residual differences are follow-up act names with correct IR/answer.
3. Frozen `adv`/`h-informal`/`dialogues` gaps are unchanged; no overfitting.
4. Two frozen UF-ranking rows still disagree on synthetic UF key `27` (pre-existing backend normalization).
5. All items synthetic; no observed questions, customers, interviews or correlation coefficients were invented.
6. IBGE CEMPRE/PAM/PPM and statistical coefficients remain fail-closed; human validation deferred.

## 7. Delivery

- Branch `codex/commercial-round-13`.
- National per-item outputs and the 98 regular batches are outside Git under `/data/opencnpj-data/intelligence/results/commercial-round-13/`.
- Merged `questions.jsonl`/`questions.tsv` and all evidence/hashes are versioned.
