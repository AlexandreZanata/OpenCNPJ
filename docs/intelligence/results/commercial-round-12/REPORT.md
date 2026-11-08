# Commercial Evaluation Round 12 — Final Report

Review date: 2026-11-08. Data anchor: `2026-10-11`; competence `2026-10`.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable completion index: [index.json](index.json).
Dataset card: [DATASET-CARD.md](../../../../crates/opencnpj-lab/eval/commercial-round-12/DATASET-CARD.md).

## 1. Executive Summary

- **Dataset size and batch structure**: **50,000** pt-BR commercial questions/turns authored across **99 batches** (`batch-001.jsonl` pilot with **1,000** items, then `batch-002.jsonl` .. `batch-099.jsonl` with **500** items each).
- **Independent gold-label audit (`dataset-audit.json`)**: **100% (50,000 / 50,000)** passed deterministic source-rule, calendar, catalogue-ID, filter-inventory, meta-schema, duplicate and leakage checks; **0** normalized overlap against the **260,000** archived texts of rounds 01–11 and the frozen suites; **0** internal duplicates; **0** cross-split scenario/skeleton/session/CNPJ-root leakage; all **142 / 142** catalogued Mato Grosso municipalities exercised.
- **Pilot (`batch-001.jsonl`, 1,000 items)**:
  - Initial draft before fixes (`pilot-before-fix/summary.json`): `interpretation_correct = 994 / 1,000`, `decision_correct = 996 / 1,000`, `gold_backend_ok = 854 / 854`.
  - Final pilot after fixes (`pilot-after-fix/summary.json`): `interpretation_correct = 1,000 / 1,000`, `decision_correct = 1,000 / 1,000`, `act_correct = 805 / 834`, `gold_backend_ok = 854 / 854`, `correct_executed_answer = 854 / 854`.
- **Full 50,000-item national evaluation (`national/summary.json`)**:
  - `items`: **50,000**
  - `interpretation_correct` (frozen labels): **50,000 / 50,000 (100.00%)**
  - `decision_correct`: **50,000 / 50,000 (100.00%)**
  - `gold_backend_ok`: **42,700 / 42,700 (100.00%)**
  - `correct_executed_answer`: **42,700 / 42,700 (100.00%)**
  - `act_correct`: **40,244 / 41,700 (96.51%)**
  - Execution provenance: `execution_fresh = 40,932`; `execution_reused_identical_ir = 1,768`.
  - Decisions: `execute = 42,700`, `clarify = 4,400`, `reject = 2,900`.
- **Differential backend verification across the 42,700 executable requests**:
  - `verification_engine_match`: **27,200**
  - `verification_single_backend`: **12,000**
  - `verification_no_independent_reference`: **2,255**
  - `verification_marginal_match`: **245**
  - `verification_not_numeric`: **1,000**
  - `verification_mismatch`: **0**
- **Cargo delivery gate**: `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (**47 / 47** NLU unit tests plus all workspace suites, **0** failures) and `cargo build --offline --release -p opencnpj-lab` all exited with code `0`.

## 2. Quotas, Splits, Regional Coverage and Provenance

All quotas and diversity rules from `ROUND-09-EXPANSION.md` were preserved exactly.

| Provenance class | Items | Correct |
|---|---:|---:|
| `synthetic_regional_grounded` | 30,000 | 30,000 |
| `synthetic_business_scenario` | 20,000 | 20,000 |
| `public_observed_adapted` | 0 | 0 |
| **Total** | **50,000** | **50,000** |

No private logs, interviews or quotes were claimed; all items are explicitly synthetic.

### 2.1 Scenario family quotas

| Primary family | Quota | Authored | Raw correct | Gold backend OK |
|---|---:|---:|---:|---:|
| `market_size_density_competition` | 5,000 | 5,000 | 5,000 | 4,000 |
| `prospecting_qualification_territory` | 7,000 | 7,000 | 7,000 | 7,000 |
| `historical_stock_flows_seasonality` | 5,000 | 5,000 | 5,000 | 5,000 |
| `geography_catchments_comparisons` | 5,000 | 5,000 | 5,000 | 5,000 |
| `corporate_structure_hq_branches_identity` | 4,000 | 4,000 | 4,000 | 4,000 |
| `supply_chain_complementary` | 5,000 | 5,000 | 5,000 | 5,000 |
| `cross_variable_correlation` | 5,000 | 5,000 | 5,000 | 3,900 |
| `dialogue_corrections_references_planning` | 6,000 | 6,000 | 6,000 | 5,800 |
| `source_quality_provenance_uncertainty` | 3,000 | 3,000 | 3,000 | 1,000 |
| `unsupported_missing_contradictions` | 3,000 | 3,000 | 3,000 | 0 |
| `tax_size_legal_capital_numeric` | 2,000 | 2,000 | 2,000 | 2,000 |
| **Total** | **50,000** | **50,000** | **50,000** | **42,700** |

### 2.2 Geography, registers, splits and diversity

- **Geography**: MT `30,000` (5,000 metropolitan, 6,000 north, 5,000 south, 4,500 east, 4,500 west, 5,000 Pantanal); other 26 UFs `12,000` (per-UF range **405–520**; every UF `>= 400`); national/cross-UF `8,000`.
- **Municipalities**: **3,357** distinct; **142 / 142** MT.
- **Registers**: `conversational_professional` 35,000; `formal_technical` 10,000; `noisy_short` 5,000.
- **Splits**: `synthetic_dev` 35,000; `synthetic_eval` 10,000; `synthetic_challenge` 5,000.
- **Floors**: MT-context **34,141**; other-UF regional **6,013**; novel semantic constructions **19,568**; clarify/refusal outcomes **7,300**.

## 3. General Interpreter Corrections

1. **Sector colloquialisms**: added `retífica`, `lojinha`, `doceria`, `pastelaria`, `choperia` and `empório` to existing sectors.
2. **Three-municipality comparison**: `Coloque lado a lado ...` is a general compare cue; the geography family now includes a three-city stock comparison.
3. **Capital boundary guard**: `No cruzamento entre porte e capital ... capital social acima de X` no longer mistakes the discourse `entre` for a two-bound interval; `entre R$ 1.000,50 e R$ 2.500,75` still parses as a range.
4. **Correlation age/status**: cross-variable now has nine subvariants, adding an active-stock change between a past competence and the current snapshot.
5. **Dialogue homonym flow**: an eighth three-turn pattern clarifies a homonym, resolves it with an explicit UF and then qualifies a head-office phone list.
6. **Three-city coverage**: new compare subvariant for MT, other-UF and national strata without losing the UF accounting slot.

## 4. Metamorphic Consistency on Real Data

`finalize_round12.py` verified metamorphic invariants (`metamorphic-checks.json`, status **PASS**, **0 violations**):

| Relation family | Checked | Passed | Violations |
|---|---:|---:|---:|
| Equivalence groups (identical canonical IR) | 1,061 groups (2,829 items) | 1,061 | 0 |
| Monotonic subset | 1,215 pairs | 1,215 | 0 |
| Partition sums | 1,400 breakdowns | 1,400 | 0 |
| Negation/exclusion bounds | 1,200 items | 1,200 | 0 |
| Temporal interval ordering | 0 observed pairs | 0 | 0 |
| Ranking prefix consistency | 1,505 pairs | 1,505 | 0 |

## 5. Frozen Suites and Prior-Round Replays

`H-blind` was never opened or executed. Rounds 01–11 remain byte-identical.

| Frozen suite | Items | Round 11 | Round 12 | Round 12 `decision_correct` |
|---|---:|---:|---:|---:|
| `adv.jsonl` | 304 | 214 / 304 | **214 / 304** | 285 / 304 |
| `h-informal.jsonl` | 202 | 121 / 202 | **121 / 202** | 172 / 202 |
| `dialogues.jsonl` | 122 | 84 / 122 | **84 / 122** | 106 / 122 |
| `ood.jsonl` | 400 | 400 / 400 | **400 / 400** | 400 / 400 |

Prior-round replays: `01` 1,000/1,000; `02` 950/1,000; `03` 2,844/3,000; `04` 4,826/5,000;
`05` 9,654/10,000; `06` 19,312/20,000; `07` 19,312/20,000; `08` 48,259/50,000;
`09` 49,975/50,000; `10` **50,000/50,000**; `11` **50,000/50,000**.

## 6. Limitations and Next Adjustments

1. **Sector catalogue versioning debt remains**: aliases were added while the serialized version stayed `1` to preserve prior datasets/replays; a future release must bump to `2` with a migration/comparator.
2. **Act taxonomy**: `act_correct = 40,244 / 41,700`; residual differences are follow-up act names while IR and answer are correct.
3. **Frozen conversational gaps unchanged**: `adv` 214/304, `h-informal` 121/202, `dialogues` 84/122. Round 12 added general patterns but did not overfit these items.
4. **Two frozen UF-ranking differentials** (`adv-0249`, `inf-0135`) still disagree on synthetic UF key `27` (base 0), a pre-existing backend normalization issue.
5. **All items synthetic**: no observed public questions, customers, interviews, quotes, demand or correlation coefficients were invented.
6. **External sources remain unintegrated**: IBGE CEMPRE/PAM/PPM and statistical coefficients still fail closed.
7. **Human validation deferred**; `human_validated = false` on every item.

## 7. Delivery

- Branch: `codex/commercial-round-12`.
- Full per-item national outputs are outside Git under `/data/opencnpj-data/intelligence/results/commercial-round-12/national/`.
- The 98 regular batch JSONL files are retained outside Git under `/data/opencnpj-data/intelligence/results/commercial-round-12/batches-snapshot/`; merged `questions.jsonl`/`questions.tsv` are versioned.
- Hashes and reproduction commands are in `evidence-sha256.json`, `evaluation-runs.json` and `COMMANDS.md`.
