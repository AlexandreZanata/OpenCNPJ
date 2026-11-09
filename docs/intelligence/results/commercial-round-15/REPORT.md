# Commercial Evaluation Round 15 — Final Report

Review date: 2026-11-09. Data anchor: `2026-10-11`; competence `2026-10`.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable completion index: [index.json](index.json).
Dataset card: [DATASET-CARD.md](../../../../crates/opencnpj-lab/eval/commercial-round-15/DATASET-CARD.md).

## 1. Executive Summary

- **Dataset**: **50,000** pt-BR questions/turns across **99 batches** (`batch-001.jsonl` pilot with **1,000** items; 98 batches of **500**).
- **Independent audit (`dataset-audit.json`)**: **50,000 / 50,000** passed; **0** normalized overlap against the **410,000** archived texts of rounds 01–14 and frozen suites; **0** internal duplicates; **0** cross-split leakage; **142 / 142** MT municipalities.
- **Pilot**: `992 / 1,000` before fixes; **`1,000 / 1,000`** after fixes (`gold_backend_ok = 860/860`).
- **National (`national/summary.json`)**:
  - `interpretation_correct`: **50,000 / 50,000 (100.00%)**
  - `decision_correct`: **50,000 / 50,000 (100.00%)**
  - `gold_backend_ok`: **43,000 / 43,000 (100.00%)**
  - `correct_executed_answer`: **43,000 / 43,000 (100.00%)**
  - `act_correct`: **40,364 / 42,000 (96.10%)**
  - `execution_fresh = 40,840`; `execution_reused_identical_ir = 2,160`
  - Decisions: `execute = 43,000`, `clarify = 4,400`, `reject = 2,600`
- **Differential verification**: `engine_match = 27,500`; `single_backend = 12,400`; `no_independent_reference = 1,905`; `marginal_match = 195`; `not_numeric = 1,000`; **`mismatch = 0`**.
- **Cargo gate**: fmt, clippy `-D warnings`, test (**49** NLU tests + workspace, 0 failures) and release build all exit `0`.

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
| `cross_variable_correlation` | 5,000 | 5,000 | 5,000 | 4,200 |
| `dialogue_corrections_references_planning` | 6,000 | 6,000 | 6,000 | 5,800 |
| `source_quality_provenance_uncertainty` | 3,000 | 3,000 | 3,000 | 1,000 |
| `unsupported_missing_contradictions` | 3,000 | 3,000 | 3,000 | 0 |
| `tax_size_legal_capital_numeric` | 2,000 | 2,000 | 2,000 | 2,000 |
| **Total** | **50,000** | **50,000** | **50,000** | **43,000** |

- **Geography**: MT `30,000` (six strata); other 26 UFs `12,000` (per-UF **405–520**, all `>= 400`); national/cross-UF `8,000`.
- **Municipalities**: **3,387** distinct; **142 / 142** MT.
- **Registers**: 35,000 conversational, 10,000 formal, 5,000 noisy.
- **Splits**: 35,000 dev, 10,000 eval, 5,000 challenge.
- **Floors**: MT-context **34,528**; other-UF regional **5,820**; novel constructions **23,951**; clarify/reject **7,000**.

## 3. General Interpreter Corrections

1. **More sector colloquialisms**: `funileiro`, `lanterneiro`, `confeiteiro`, `doceiro` and `jardineiro` mapped to existing sectors; `confeitaria` kept under the bakery surface to avoid a frozen-suite regression.
2. **CNAE group breakdown**: `por grupo CNAE` now maps to the CNAE group hierarchy.
3. **Three-city density comparison**: geography alternates two- and three-municipality per-10k density comparisons.
4. **Bounded capital combinations**: prospecting MEI list and tax Simples/non-MEI count inside a capital interval.
5. **Build consistency**: national, frozen and replay evidence were re-run after a clean rebuild; rounds 13 and 14 returned to 50,000/50,000.

## 4. Metamorphic Consistency

`metamorphic-checks.json`: **PASS**, **0 violations** — 1,348 equivalence groups (3,508 items), 1,428 monotonic pairs, 2,600 partition sums, 1,600 exclusion bounds, 1,682 ranking prefixes.

## 5. Frozen Suites and Replays

`H-blind` was not opened or executed. Rounds 01–14 remain byte-identical.

| Frozen suite | Items | Round 14 | Round 15 | Decision correct |
|---|---:|---:|---:|---:|
| `adv.jsonl` | 304 | 214 / 304 | **214 / 304** | 285 / 304 |
| `h-informal.jsonl` | 202 | 121 / 202 | **121 / 202** | 172 / 202 |
| `dialogues.jsonl` | 122 | 84 / 122 | **84 / 122** | 106 / 122 |
| `ood.jsonl` | 400 | 400 / 400 | **400 / 400** | 400 / 400 |

Prior replays: `01` 1,000/1,000; `02` 950/1,000; `03` 2,844/3,000; `04` 4,826/5,000; `05` 9,654/10,000; `06` 19,312/20,000; `07` 19,312/20,000; `08` 48,259/50,000; `09` 49,975/50,000; `10`–`14` **50,000/50,000 each**.

## 6. Limitations and Next Adjustments

1. Sector catalogue still serialized as version `1`; aliases added without breaking prior datasets. Future release should bump to v2 with migration.
2. `act_correct = 40,364 / 42,000`; residual differences are follow-up act names with correct IR/answer.
3. Frozen `adv`/`h-informal`/`dialogues` gaps unchanged; no overfitting.
4. Two frozen UF-ranking rows still disagree on synthetic UF key `27` (pre-existing backend normalization).
5. All items synthetic; no observed questions, customers, interviews or correlation coefficients were invented.
6. IBGE CEMPRE/PAM/PPM and statistical coefficients remain fail-closed; human validation deferred.

## 7. Delivery

- Branch `codex/commercial-round-15`.
- National per-item outputs and the 98 regular batches are outside Git under `/data/opencnpj-data/intelligence/results/commercial-round-15/`.
- Merged `questions.jsonl`/`questions.tsv` and all evidence/hashes are versioned.
