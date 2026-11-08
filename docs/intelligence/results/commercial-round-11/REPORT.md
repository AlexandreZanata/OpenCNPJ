# Commercial Evaluation Round 11 — Final Report

Review date: 2026-11-08. Data anchor: `2026-10-11`; competence `2026-10`.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable completion index: [index.json](index.json).
Dataset card: [DATASET-CARD.md](../../../../crates/opencnpj-lab/eval/commercial-round-11/DATASET-CARD.md).

## 1. Executive Summary

- **Dataset size and batch structure**: **50,000** pt-BR commercial questions/turns authored across **99 batches** (`batch-001.jsonl` pilot with **1,000** items, then `batch-002.jsonl` .. `batch-099.jsonl` with **500** items each).
- **Independent gold-label audit (`dataset-audit.json`)**: **100% (50,000 / 50,000)** passed deterministic source-rule, calendar, catalogue-ID, filter-inventory, meta-schema, duplicate and leakage checks; **0** normalized overlap against the **210,000** archived texts of rounds 01–10 and the frozen suites; **0** internal duplicates; **0** cross-split scenario/skeleton/session/CNPJ-root leakage; all **142 / 142** catalogued Mato Grosso municipalities exercised.
- **Pilot (`batch-001.jsonl`, 1,000 items)**:
  - Initial draft before NLU/authoring fixes (`pilot-before-fix/summary.json`): `interpretation_correct = 984 / 1,000`, `decision_correct = 984 / 1,000`, `gold_backend_ok = 856 / 856`.
  - Final pilot after fixes (`pilot-after-fix/summary.json`): `interpretation_correct = 1,000 / 1,000`, `decision_correct = 1,000 / 1,000`, `act_correct = 808 / 836`, `gold_backend_ok = 856 / 856`, `correct_executed_answer = 856 / 856`.
- **Full 50,000-item national evaluation (`national/summary.json`)**:
  - `items`: **50,000**
  - `interpretation_correct` (frozen labels): **50,000 / 50,000 (100.00%)**
  - `decision_correct`: **50,000 / 50,000 (100.00%)**
  - `gold_backend_ok`: **42,800 / 42,800 (100.00%)**
  - `correct_executed_answer`: **42,800 / 42,800 (100.00%)**
  - `act_correct`: **40,400 / 41,800 (96.65%)** (Round 10: `39,800 / 41,000`)
  - Execution provenance: `execution_fresh = 40,962`; `execution_reused_identical_ir = 1,838`.
  - Decisions: `execute = 42,800`, `clarify = 4,200`, `reject = 3,000`.
- **Differential backend verification across the 42,800 executable requests**:
  - `verification_engine_match`: **27,800**
  - `verification_single_backend`: **12,000**
  - `verification_no_independent_reference`: **1,804**
  - `verification_marginal_match`: **196**
  - `verification_not_numeric`: **1,000**
  - `verification_mismatch`: **0**
- **Cargo delivery gate**: `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (**46 / 46** NLU unit tests plus all workspace suites, **0** failures) and `cargo build --offline --release -p opencnpj-lab` all exited with code `0`.

## 2. Quotas, Splits, Regional Coverage and Provenance

All Round 09 quotas and diversity rules were preserved exactly (`dataset-audit.json`, `split-breakdown.json`, `manifest.json`).

| Provenance class | Items | Correct |
|---|---:|---:|
| `synthetic_regional_grounded` | 30,000 | 30,000 |
| `synthetic_business_scenario` | 20,000 | 20,000 |
| `public_observed_adapted` | 0 | 0 |
| **Total** | **50,000** | **50,000** |

No private logs, interviews or quotes were claimed. No publicly observed question set with verified
reuse rights was added, so all 50,000 items are explicitly synthetic.

### 2.1 Scenario family quotas

| Primary family | Quota | Authored | Raw correct | Gold backend OK |
|---|---:|---:|---:|---:|
| `market_size_density_competition` | 5,000 | 5,000 | 5,000 | 4,000 |
| `prospecting_qualification_territory` | 7,000 | 7,000 | 7,000 | 7,000 |
| `historical_stock_flows_seasonality` | 5,000 | 5,000 | 5,000 | 5,000 |
| `geography_catchments_comparisons` | 5,000 | 5,000 | 5,000 | 5,000 |
| `corporate_structure_hq_branches_identity` | 4,000 | 4,000 | 4,000 | 4,000 |
| `supply_chain_complementary` | 5,000 | 5,000 | 5,000 | 5,000 |
| `cross_variable_correlation` | 5,000 | 5,000 | 5,000 | 3,800 |
| `dialogue_corrections_references_planning` | 6,000 | 6,000 | 6,000 | 6,000 |
| `source_quality_provenance_uncertainty` | 3,000 | 3,000 | 3,000 | 1,000 |
| `unsupported_missing_contradictions` | 3,000 | 3,000 | 3,000 | 0 |
| `tax_size_legal_capital_numeric` | 2,000 | 2,000 | 2,000 | 2,000 |
| **Total** | **50,000** | **50,000** | **50,000** | **42,800** |

### 2.2 Geography, registers, splits and diversity

- **Geography**: MT `30,000` (5,000 metropolitan, 6,000 north, 5,000 south, 4,500 east, 4,500 west, 5,000 Pantanal); other 26 UFs `12,000` (per-UF range **405–520**; every UF `>= 400`); national/cross-UF `8,000`.
- **Municipalities**: **3,078** distinct; **142 / 142** MT.
- **Registers**: `conversational_professional` 35,000; `formal_technical` 10,000; `noisy_short` 5,000.
- **Splits**: `synthetic_dev` 35,000; `synthetic_eval` 10,000; `synthetic_challenge` 5,000.
- **Floors**: MT-context **33,948**; other-UF regional **6,011**; novel semantic constructions **18,144**; clarify/refusal outcomes **7,200**.

## 3. General Interpreter Corrections

1. **More sector colloquialisms**: added `churrascaria`, `hamburgueria`, `cafeteria`, `sorveteria` and `adega` to the existing restaurant/snack-bar/beverage sectors and exercised them as surfaces.
2. **Compound act taxonomy**: simultaneous keep/drop edits (`mantenha o filtro de matriz, tire o MEI`) now return the general `compound_edit` act instead of `remove_filter`.
3. **Exclusion carry-over**: a follow-up that does not restate a place exclusion now preserves the inherited exclusion instead of silently clearing it.
4. **Count imperatives**: `conte o estado...`, `conte a região...` and similar article forms no longer trigger the unknown-sector clarification.
5. **Capital extraction**: when `capital social` appears twice (for example in a discourse frame and again with the number), the parser binds the value to the occurrence that actually carries a money quantity.
6. **Discourse pre-check**: `No cruzamento ...` is recognized as a discourse frame, not as the preposition `no` plus an unknown place.
7. **Correlation subvariants**: cross-variable authoring now has eight subvariants (population density, net flow, size/MEI share, external reject, capital vs size, branch penetration with population band, secondary-CNAE co-occurrence, statistical reject).
8. **Dialogues**: seven three-turn patterns now cover compound add/remove/switch, prefix resolution, state exclusions and sector pivots.

## 4. Metamorphic Consistency on Real Data

`finalize_round11.py` verified metamorphic invariants across the executed results (`metamorphic-checks.json`, status **PASS**, **0 violations**):

| Relation family | Checked | Passed | Violations |
|---|---:|---:|---:|
| Equivalence groups (identical canonical IR) | 1,224 groups (3,062 items) | 1,224 | 0 |
| Monotonic subset (restrictive filters ≤ superset) | 1,315 pairs | 1,315 | 0 |
| Partition sums (breakdown rows == reference/base) | 1,400 breakdowns | 1,400 | 0 |
| Negation/exclusion bounds (`0 <= excluded <= base`) | 1,500 items | 1,500 | 0 |
| Temporal interval ordering | 0 observed pairs | 0 | 0 |
| Ranking prefix consistency | 1,687 pairs | 1,687 | 0 |

## 5. Frozen Suites and Prior-Round Replays

`H-blind` was never opened or executed. Rounds 01–10 on disk remain byte-identical.

| Frozen suite | Items | Round 10 | Round 11 | Round 11 `decision_correct` |
|---|---:|---:|---:|---:|
| `adv.jsonl` | 304 | 214 / 304 | **214 / 304** | 285 / 304 |
| `h-informal.jsonl` | 202 | 121 / 202 | **121 / 202** | 172 / 202 |
| `dialogues.jsonl` | 122 | 84 / 122 | **84 / 122** | 106 / 122 |
| `ood.jsonl` | 400 | 400 / 400 | **400 / 400** | 400 / 400 |

Prior-round replays: `01` 1,000/1,000; `02` 950/1,000; `03` 2,844/3,000; `04` 4,826/5,000;
`05` 9,654/10,000; `06` 19,312/20,000; `07` 19,312/20,000; `08` 48,259/50,000;
`09` 49,975/50,000; `10` **50,000/50,000**.

## 6. Limitations and Next Adjustments

1. **Sector catalogue versioning debt remains**: aliases were added while the serialized catalogue version stayed `1` to keep prior datasets and replays byte-identical. A future release must bump to `2` with a migration/comparator.
2. **Act taxonomy**: `act_correct = 40,400 / 41,800`; remaining differences are follow-up act names while IR and answer are correct.
3. **Frozen conversational gaps unchanged**: `adv` 214/304, `h-informal` 121/202, `dialogues` 84/122. Round 11 added general patterns but did not overfit these items.
4. **Two frozen UF-ranking differentials** (`adv-0249`, `inf-0135`) still disagree on the synthetic UF key `27` (base 0). This is a pre-existing backend normalization issue.
5. **All items synthetic**: no observed public questions, customers, interviews, quotes, demand or correlation coefficients were invented.
6. **External sources remain unintegrated**: IBGE CEMPRE/PAM/PPM and statistical coefficients still fail closed.
7. **Human validation deferred**; `human_validated = false` on every item.

## 7. Delivery

- Branch: `codex/commercial-round-11`.
- Full per-item national outputs are outside Git under `/data/opencnpj-data/intelligence/results/commercial-round-11/national/`.
- The 98 regular batch JSONL files are retained outside Git under `/data/opencnpj-data/intelligence/results/commercial-round-11/batches-snapshot/`; merged `questions.jsonl`/`questions.tsv` are versioned.
- Hashes and reproduction commands are in `evidence-sha256.json`, `evaluation-runs.json` and `COMMANDS.md`.
