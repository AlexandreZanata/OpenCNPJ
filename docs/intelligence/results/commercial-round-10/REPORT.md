# Commercial Evaluation Round 10 — Final Report

Review date: 2026-11-08. Data anchor: `2026-10-11`; competence `2026-10`.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable completion index: [index.json](index.json).
Dataset card: [DATASET-CARD.md](../../../../crates/opencnpj-lab/eval/commercial-round-10/DATASET-CARD.md).

## 1. Executive Summary

- **Dataset size and batch structure**: **50,000** pt-BR commercial questions/turns authored across **99 batches** (`batch-001.jsonl` pilot with **1,000** items, then `batch-002.jsonl` .. `batch-099.jsonl` with **500** items each).
- **Independent gold-label audit (`dataset-audit.json`)**: **100% (50,000 / 50,000)** passed deterministic source-rule, calendar, catalogue-ID, filter-inventory, meta-schema, duplicate and leakage checks; **0** overlap against the **160,000** archived texts of rounds 01–09 and the frozen suites; **0** internal folded duplicates; **0** cross-split scenario/skeleton/session/CNPJ-root leakage; all **142 / 142** catalogued Mato Grosso municipalities exercised.
- **Pilot (`batch-001.jsonl`, 1,000 items)**:
  - Initial draft before NLU/authoring fixes (`pilot-before-fix/summary.json`): `interpretation_correct = 813 / 1,000`, `decision_correct = 936 / 1,000`, `gold_backend_ok = 840 / 840`.
  - Final pilot after fixes (`pilot-after-fix/summary.json`): `interpretation_correct = 1,000 / 1,000`, `decision_correct = 1,000 / 1,000`, `act_correct = 796 / 820`, `gold_backend_ok = 840 / 840`, `correct_executed_answer = 840 / 840`.
- **Full 50,000-item national evaluation (`national/summary.json`)**:
  - `items`: **50,000**
  - `interpretation_correct` (frozen labels): **50,000 / 50,000 (100.00%)**
  - `decision_correct`: **50,000 / 50,000 (100.00%)**
  - `gold_backend_ok`: **42,000 / 42,000 (100.00%)** (`0` invalid gold IRs, `0` backend errors on gold)
  - `correct_executed_answer`: **42,000 / 42,000 (100.00%)**
  - `act_correct`: **39,800 / 41,000 (97.07%)**
  - Execution provenance: `execution_fresh = 40,286`; `execution_reused_identical_ir = 1,714`.
  - Decisions: `execute = 42,000`, `clarify = 4,200`, `reject = 3,800`.
- **Differential backend verification across the 42,000 executable requests**:
  - `verification_engine_match`: **28,300**
  - `verification_single_backend`: **10,700**
  - `verification_no_independent_reference`: **1,806**
  - `verification_marginal_match`: **194**
  - `verification_not_numeric`: **1,000**
  - `verification_mismatch`: **0**
- **Cargo delivery gate**: `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (**45 / 45** NLU unit tests plus all workspace suites, **0** failures) and `cargo build --offline --release -p opencnpj-lab` all exited with code `0`.

## 2. Quotas, Splits, Regional Coverage and Provenance

All Round 09 quotas and diversity rules were preserved exactly (`dataset-audit.json`, `split-breakdown.json`, `manifest.json`).

### 2.1 Provenance mix

| Provenance class | Items | Correct |
|---|---:|---:|
| `synthetic_regional_grounded` (MT or UF regional context) | 30,000 | 30,000 |
| `synthetic_business_scenario` (national/cross-UF) | 20,000 | 20,000 |
| `public_observed_adapted` | 0 | 0 |
| **Total** | **50,000** | **50,000** |

No private customer logs, interviews, quotes or proprietary tickets were claimed. No new publicly
observable question set with verified reuse rights was added in this round, so **all 50,000 items are
explicitly synthetic**; regional wording is grounded in the documented sources in
`regional-glossary.json` but does not claim that any entrepreneur asked the authored question.

### 2.2 Scenario family quotas (11 families)

| Primary family | Quota | Authored | Raw correct | Gold backend OK |
|---|---:|---:|---:|---:|
| `market_size_density_competition` | 5,000 | 5,000 | 5,000 | 4,000 |
| `prospecting_qualification_territory` | 7,000 | 7,000 | 7,000 | 7,000 |
| `historical_stock_flows_seasonality` | 5,000 | 5,000 | 5,000 | 5,000 |
| `geography_catchments_comparisons` | 5,000 | 5,000 | 5,000 | 5,000 |
| `corporate_structure_hq_branches_identity` | 4,000 | 4,000 | 4,000 | 4,000 |
| `supply_chain_complementary` | 5,000 | 5,000 | 5,000 | 5,000 |
| `cross_variable_correlation` | 5,000 | 5,000 | 5,000 | 3,000 |
| `dialogue_corrections_references_planning` | 6,000 | 6,000 | 6,000 | 6,000 |
| `source_quality_provenance_uncertainty` | 3,000 | 3,000 | 3,000 | 1,000 |
| `unsupported_missing_contradictions` | 3,000 | 3,000 | 3,000 | 0 |
| `tax_size_legal_capital_numeric` | 2,000 | 2,000 | 2,000 | 2,000 |
| **Total** | **50,000** | **50,000** | **50,000** | **42,000** |

### 2.3 Geography, registers, splits and diversity floors

- **Geography**:
  - `mt_metropolitan`: 5,000
  - `mt_north_agro_industrial`: 6,000
  - `mt_south_logistics_industrial`: 5,000
  - `mt_east_araguaia_xingu`: 4,500
  - `mt_west_parecis_guapore`: 4,500
  - `mt_pantanal`: 5,000
  - `other_26_ufs`: 12,000 (per-UF range **405–520**; every UF `>= 400`)
  - `national_cross_uf_comparative`: 8,000
- **Municipalities exercised**: **3,066** distinct; **142 / 142** MT municipalities.
- **Registers**: `conversational_professional` 35,000; `formal_technical` 10,000; `noisy_short` 5,000.
- **Splits**: `synthetic_dev` 35,000; `synthetic_eval` 10,000; `synthetic_challenge` 5,000, partitioned by scenario/skeleton/session/CNPJ root.
- **Overlapping floors**: MT-context **34,250** (`>= 8,000`); other-UF regional **5,624** (`>= 4,000`); novel semantic constructions **16,831** (`>= 15,000`); clarification/refusal outcomes **8,000** (`>= 5,000`).
- **Novelty tags** include compound multi-turn scope edits (6,000), exact-name municipal prefix resolution (2,390), colloquial sector surfaces (~1,000), opened→closure cohort boundaries (1,000), state-internal exclusions (1,000), CNAE section/division breakdowns (1,000), population-band density rankings (1,000), sector-by-tax-regime shares (1,000), and documentation/validation guards (2,000).

## 3. General Interpreter Corrections and Regressions

The round attacked the three remaining Round 09 gaps and fixed general defects found by the pilot and national run:

1. **Sector colloquialisms (`crates/opencnpj-insights/catalog/sectors.toml`)**:
   - Added documented commercial aliases: `oficina`/`mecânica` → automotive repair, `posto` → fuel station, `funilaria` → body shop, `pizzaria` → restaurant, `pet` → pet shop, `gráfica` → advertising printing, `eletricista` → electrical installation.
   - Added the `meat_processing` sector (`frigorífico`, `abatedouro`, and slaughter CNAEs 1011*/1012*), which was absent from the catalogue.
   - `clínica` and `revenda` without a concrete activity remain explicit clarifications.
2. **Abbreviated list imperatives and limits (`cues.toml`, `grammar.rs`)**:
   - `mostre as`, `mostra as`, `mostre os`, `mostra os`, `me passa a lista`, `me passe a lista`.
   - `lista/listagem/relação de N` now binds an explicit list limit.
3. **Compound multi-turn scope edits (`grammar.rs`)**:
   - `mantenha o filtro de matriz` now survives a simultaneous `tire o MEI`, instead of clearing every dimension mentioned in the turn.
   - Added three-turn dialogues combining keep/drop/switch edits, state exclusions, sector pivots and focus-based comparisons.
4. **Municipal prefix disambiguation**:
   - Exact single-word names resolve before longer prefix-sharing names (`Cruzeiro` → Cruzeiro/SP; `Cruzeiro do Oeste` → its own municipality; genuine exact homonyms such as `Cruzeiro do Sul` still clarify).
   - Regression tests cover `Cruzeiro`, `Cruzeiro do Oeste`, `Cruzeiro do Sul`, `Santa Rosa`, `Santa Rosa da Serra`.
   - Removed the stale `Cruzeiro` homonym clarification list that caused the 25 Round 09 adjudications.
5. **State-name / cue collisions (`grammar.rs`)**:
   - The abundance cue `é mato` no longer matches the state name in `... e Mato Grosso`.
   - Lowercase `para` is accepted as Pará in explicit plural state lists (`estados de para e mato grosso`) without turning the preposition into a state.
   - Preposition `entre` inside a municipality name (`Desterro de Entre Rios`) no longer forces a two-bound capital range.
6. **Domain and filter precision**:
   - Sector rankings (`Quais os 10 setores ...`) are treated as cadastral domain instead of being rejected.
   - Contact presence remains a list-only filter; the dialogue that requested phone-qualified head offices was converted to a list rather than an invalid count.

## 4. Metamorphic Consistency on Real Data

`finalize_round10.py` verified metamorphic invariants across the executed results (`metamorphic-checks.json`, status **PASS**, **0 violations**):

| Relation family | Checked | Passed | Violations |
|---|---:|---:|---:|
| Equivalence groups (identical canonical IR → identical value/prediction) | 1,067 groups (2,781 items) | 1,067 | 0 |
| Monotonic subset (more restrictive filters → count ≤ superset) | 1,169 pairs | 1,169 | 0 |
| Partition sums (breakdown rows == reference/base) | 1,400 breakdowns | 1,400 | 0 |
| Negation/exclusion bounds (`0 <= excluded <= base`) | 1,400 items | 1,400 | 0 |
| Temporal interval ordering (nested flow windows) | 0 observed pairs | 0 | 0 |
| Ranking prefix consistency (`top k1` is a prefix of `top k2`) | 1,668 pairs | 1,668 | 0 |

## 5. Independent Frozen Suites and Prior-Round Replays

`H-blind` was never opened or executed. Rounds 01–09 on disk are byte-identical to their prior commits.

### 5.1 Frozen suites (unchanged from Round 09)

| Frozen suite | Items | Round 09 `interpretation_correct` | Round 10 `interpretation_correct` | Round 10 `decision_correct` |
|---|---:|---:|---:|---:|
| `adv.jsonl` | 304 | 214 / 304 | **214 / 304** | 285 / 304 |
| `h-informal.jsonl` | 202 | 121 / 202 | **121 / 202** | 172 / 202 |
| `dialogues.jsonl` | 122 | 84 / 122 | **84 / 122** | 106 / 122 |
| `ood.jsonl` | 400 | 400 / 400 | **400 / 400** | 400 / 400 |

Two frozen execution comparisons (`adv-0249`, `inf-0135`, UF ranking "por habitante") still show one
differential backend mismatch each: one path emits the synthetic UF key `27` (base 0) and the other
does not. This is a pre-existing backend discrepancy, not a Round 10 interpretation change; it is
listed under limitations.

### 5.2 Prior commercial rounds replay (`commercial-round-01` .. `commercial-round-09`)

| Prior round | Items | `interpretation_correct` (raw frozen) |
|---|---:|---:|
| `commercial-round-01` | 1,000 | **1,000 / 1,000** |
| `commercial-round-02` | 1,000 | **950 / 1,000** |
| `commercial-round-03` | 3,000 | **2,844 / 3,000** |
| `commercial-round-04` | 5,000 | **4,826 / 5,000** |
| `commercial-round-05` | 10,000 | **9,654 / 10,000** |
| `commercial-round-06` | 20,000 | **19,312 / 20,000** |
| `commercial-round-07` | 20,000 | **19,312 / 20,000** |
| `commercial-round-08` | 50,000 | **48,259 / 50,000** |
| `commercial-round-09` | 50,000 | **49,975 / 50,000** |

Round 08's 1,600 closure adjudication patch and Round 09's 25 `Cruzeiro` adjudications remain
documented in their own result directories; no frozen original was edited.

## 6. Limitations and Next Adjustments

1. **Sector catalogue versioning debt**: `sectors.toml` gained aliases and a `meat_processing`
   sector while the serialized catalogue version stayed `1`. Bumping it would invalidate the
   byte-identical prior-round question/gold versions and replay comparisons. The next release
   should bump to `2` with a migration for prior gold IR or a version-aware comparator.
2. **Act taxonomy**: `act_correct = 39,800 / 41,000`. Remaining differences are follow-up act
   names (for example `remove_filter` vs authored `correction`) while the canonical IR and answer
   are correct. The act vocabulary should be tightened without changing semantics.
3. **Frozen-suite differential**: the two UF-ranking "por habitante" comparisons described above
   need a backend normalization decision for synthetic UF key `27`.
4. **Frozen conversational gaps persist**: `adv` 214/304, `h-informal` 121/202 and `dialogues`
   84/122. Round 10 deliberately did not overfit these items; colloquial sector coverage and
   implicit 4+ turn referent carry-over remain product-refinement work.
5. **All items are synthetic**: zero observed public questions were added. No customer, interview,
   quote, demand or correlation coefficient was invented.
6. **External data still not integrated**: IBGE CEMPRE, PAM and PPM requests still fail closed;
   no coefficient, revenue, headcount or supplier link is fabricated.
7. **Prior-text count discrepancy**: the brief cites `159,000` prior questions for rounds 01–09;
   the archived files contain `160,000` (`1,000+1,000+3,000+5,000+10,000+20,000+20,000+50,000+50,000`).
   The overlap audit checked all `160,000` and found zero normalized overlap.
8. **Human validation remains deferred** to product refinement; `human_validated = false` on
   every item. The synthetic splits are not H-blind and are not human validation.

## 7. Delivery

- Branch: `codex/commercial-round-10`.
- Commits and pushed revision are listed in the final response and in `index.json` (`source_revision`).
- Full per-item `results.jsonl` and `predictions.jsonl` were kept outside Git under
  `/data/opencnpj-data/intelligence/results/commercial-round-10/national/`; hashes and
  reproduction commands are in `evidence-sha256.json`, `evaluation-runs.json` and `COMMANDS.md`.
- The 98 regular batch JSONL files are retained outside Git in
  `/data/opencnpj-data/intelligence/results/commercial-round-10/batches-snapshot/`; their hashes
  are in `batch-manifests.json`, and the merged `questions.jsonl`/`questions.tsv` are versioned.
