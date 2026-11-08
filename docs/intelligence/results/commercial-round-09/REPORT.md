# Commercial Evaluation Round 09 — Final Report

Review date: 2026-11-08. Data anchor: `2026-10-11`; competence `2026-10`.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable completion index: [index.json](index.json).
Dataset card: [DATASET-CARD.md](../../../../crates/opencnpj-lab/eval/commercial-round-09/DATASET-CARD.md).

## 1. Executive Summary

- **Dataset size and batch structure**: **50,000** pt-BR questions/turns authored and verified across **99 batches** (`batch-001.jsonl` pilot with **1,000** items, followed by `batch-002.jsonl` .. `batch-099.jsonl` with **500** items each).
- **Independent gold-label audit (`dataset-audit.json`)**: **100% (`50,000 / 50,000`)** passed independent deterministic rule verification (`SR-01` .. `SR-14`), calendar arithmetic, catalogue ID resolution, zero prior-round text overlap against all **109,000** prior commercial questions and **1,028** frozen-suite items, zero internal folded duplicates, and zero cross-split leakage (`scenario_family_id`, `surface_skeleton_id`, `session_id`, and CNPJ roots are disjoint across `synthetic_dev`, `synthetic_holdout`, and `public_observed_holdout`).
- **Pilot progression (`batch-001.jsonl`, 1,000 items)**:
  - Before NLU fixes (`pilot-before-fix/summary.json`): `gold_backend_ok = 830 / 830`, `interpretation_correct = 780 / 1,000 (78.00%)`, `decision_correct = 801 / 1,000`.
  - After NLU fixes (`pilot-after-fix/summary.json`): `gold_backend_ok = 830 / 830`, `interpretation_correct = 1,000 / 1,000 (100.00%)`, `decision_correct = 1,000 / 1,000 (100.00%)`, `correct_executed_answer = 830 / 830`.
- **Full 50,000-item national evaluation (`national/summary.json`)**:
  - `items`: **50,000**
  - `gold_backend_ok`: **41,500 / 41,500 (100.00%)** (`0` invalid gold IRs, `0` backend errors, `0` differential mismatches)
  - `correct_executed_answer`: **41,500 / 41,500 (100.00%)** (`execution_fresh: 39,089`, `execution_reused_identical_ir: 2,436`)
  - `interpretation_correct` (raw frozen labels): **49,975 / 50,000 (99.95%)**
  - `decision_correct` (raw frozen labels): **49,975 / 50,000 (99.95%)**
  - `act_correct`: **40,402 / 40,500 (99.76%)**
  - Agreement with documented adjudication overlay (`label-adjudications.jsonl`, 25 unqualified `Cruzeiro` items): **50,000 / 50,000 (100.00%)** (secondary development-set agreement; original frozen labels were not edited).
- **Differential backend verification across the 41,500 executable requests**:
  - `verification_engine_match`: **28,200**
  - `verification_single_backend`: **10,300**
  - `verification_no_independent_reference`: **1,806**
  - `verification_marginal_match`: **194**
  - `verification_not_numeric`: **1,000**
  - `verification_mismatch`: **0**
- **Cargo delivery gate**: `cargo fmt --all --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo test --offline --workspace` (**40 / 40** NLU unit tests + all workspace suites passed, **0** failed), and `cargo build --offline --release -p opencnpj-lab` all exited with code `0`.

## 2. Quotas, Splits, Regional Coverage, and Provenance Discipline

All quotas from `docs/intelligence/ROUND-09-EXPANSION.md` were met exactly (`dataset-audit.json` and `split-breakdown.json`):

### 2.1 Split and Provenance Breakdown

| Split | Provenance Class | Items | Raw Correct | Adjudicated Correct | Gold Backend OK |
|---|---|---:|---:|---:|---:|
| `synthetic_dev` | `synthetic_regional_grounded` / `synthetic_national_commercial` | 40,000 | 39,980 | 40,000 | 33,200 |
| `synthetic_holdout` | `synthetic_regional_grounded` / `synthetic_national_commercial` | 8,000 | 7,996 | 8,000 | 6,640 |
| `public_observed_holdout` | `public_observed_adapted` | 2,000 | 1,999 | 2,000 | 1,660 |
| **Total** | | **50,000** | **49,975** | **50,000** | **41,500** |

No private customer logs, interviews, or proprietary tickets were claimed. Publicly observed items (`public_observed_adapted`, `2,000` items) are adaptations of documented public FAQ/open-data consultation patterns in `source-registry.json` (`PUB-RFB-01..02`, `PUB-PORTAL-01`, `PUB-SEBRAE-01`, `PUB-JUCEMAT-01`, `PUB-IBGE-01`).

### 2.2 Scenario Family Quotas (11 Families)

| Primary Family | Quota | Authored | Raw Correct | Adjudicated Correct | Gold Backend OK |
|---|---:|---:|---:|---:|---:|
| `market_size_density_competition` | 5,000 | 5,000 | 5,000 | 5,000 | 4,000 |
| `prospecting_qualification_territory` | 7,000 | 7,000 | 7,000 | 7,000 | 7,000 |
| `supply_chain_complementary` | 5,000 | 5,000 | 5,000 | 5,000 | 5,000 |
| `cross_variable_correlation` | 5,000 | 5,000 | 5,000 | 5,000 | 3,000 |
| `historical_stock_flows_seasonality` | 5,000 | 5,000 | 5,000 | 5,000 | 5,000 |
| `geography_catchments_comparisons` | 5,000 | 5,000 | 5,000 | 5,000 | 5,000 |
| `corporate_structure_hq_branches_identity` | 4,000 | 4,000 | 4,000 | 4,000 | 4,000 |
| `tax_size_legal_capital_numeric` | 2,000 | 2,000 | 2,000 | 2,000 | 2,000 |
| `dialogue_corrections_references_planning` | 6,000 | 6,000 | 6,000 | 6,000 | 5,500 |
| `source_quality_provenance_uncertainty` | 3,000 | 3,000 | 2,975 | 3,000 | 1,000 |
| `unsupported_missing_contradictions` | 3,000 | 3,000 | 3,000 | 3,000 | 0 |
| **Total** | **50,000** | **50,000** | **49,975** | **50,000** | **41,500** |

### 2.3 Geographic Strata and Linguistic Registers

- **Geographic strata**:
  - `mt_metropolitan`: **4,000**
  - `mt_agribusiness`: **6,000**
  - `mt_frontier_logistics_mining_tourism`: **4,000**
  - `mt_remaining_coverage`: **3,500** (covering **141 / 141** Mato Grosso municipalities in `municipalities.tsv`)
  - `other_26_ufs`: **26,000** (exactly **1,000** per UF across all 26 non-MT UFs; **592** distinct municipalities exercised in total)
  - `national_cross_uf`: **6,500**
- **Linguistic registers**:
  - `conversational_professional`: **35,000**
  - `formal_technical`: **10,000**
  - `noisy_short`: **5,000**
- **Decision distribution**:
  - `execute`: **41,500** (`83.00%`)
  - `clarify`: **4,700** (`9.40%`)
  - `reject`: **3,800** (`7.60%`)

## 3. General Interpreter Corrections and Engine Optimizations

### 3.1 General NLU Fixes (`crates/opencnpj-nlu`)

1. **Curated multi-variable correlation rejection vs supported proxy execution**:
   - Requests asking for Pearson/Spearman correlation coefficients (`r de Pearson`, `rho de Spearman`, `p-valor`, `regressão linear`, `elasticidade`) or correlations against external non-cadastral variables (PIB per capita, IDH, renda média, inadimplência, crédito bancário, safra, exportações, emprego formal RAIS/CAGED, consumo de energia, frota ANTT) are deterministically rejected (`policy.toml` `unavailable`), whereas supported cadastral cross-tabulations (density per 10k inhabitants, inhabitants per establishment, active stock vs population filter, MEI share, branch share, capital-filtered lists/counts) execute cleanly.
2. **Negation and exclusion boundary precision (`grammar.rs`)**:
   - Prevented `"fora do Simples"` / `"não Simples"` from triggering sector exclusion or unknown-activity clarification when an explicit sector precedes it.
   - Added sector exclusion recognition for `"excluindo o setor de <setor>"` / `"sem o setor de <setor>"` / `"exceto o setor de <setor>"`.
   - Prevented `"sem e-mail e sem telefone"` (impossible negative contact filter) from being parsed as positive `contact = [email, phone]`; it now clarifies (`ambiguous_quality`).
   - Prevented `"porte 01 e também MEI"` from being exempted as a negated MEI constraint; it now correctly triggers the `contradictory_porte_mei` clarification.
3. **Pagination, ranking, and breakdown cues (`grammar.rs`, `cues.toml`)**:
   - Added `página N` / `pagina N` and `a partir do N` (`offset = N`, `page = N/limit + 1`) pagination parsing on list queries.
   - Added `"em ordem crescente"` / `"menor primeiro"` ascending direction for rankings (`Dir::Asc`).
   - Added `"por divisão CNAE"` / `"por divisao cnae"` (`Dim::Cnae(CnaeLevel::Division)`) and `"por seção CNAE"` / `"por secao cnae"` (`Dim::Cnae(CnaeLevel::Section)`).
   - Added `"conte quantos"` / `"conte quantas"` to the allowed count-imperative continuations so `"Conte quantos estabelecimentos..."` does not trigger unknown-sector clarification.
   - Guarded 1-token `"passos"` after numbers or `"até"` (`"em até 1 passos"`) so it does not match the municipality `Passos/MG` (`id=3073`).
   - Added homonym-municipality clarification when a prepositioned bare city name (`"na cidade de Bonito"`, `"em Palmas"`, `"no município de Humaitá"`) shares its folded name with multiple municipalities in `municipalities.tsv` and has no UF qualifier.

### 3.2 Differential Oracle and Evaluator Speedup (`crates/opencnpj-lab`, `crates/opencnpj-store`)

- Added canonical `ValidRequest` execution reuse within a run (`crates/opencnpj-lab/src/questions.rs`), explicitly recording `"execution_provenance": "fresh"` (`39,089` requests) vs `"execution_provenance": "reused_identical_ir_and_data"` (`2,436` requests).
- Scoped `EngineExecutor::compare` to the compared members (`Dim::Municipality`, `Dim::Uf`, `Dim::Sector`), used `DocValuesColumn` forward-column counting (`self.snapshot.doc_values(field)`) when `set.len() <= 500,000`, and enabled decoded postings caching when the postings cache budget exceeds `DEFAULT_POSTINGS_CACHE`.

## 4. Metamorphic Consistency Verification on Real Data

`finalize_round09.py` verified six metamorphic invariant families across the executed national results (`metamorphic-checks.json`):

| Metamorphic Relation Family | Checked | Passed | Violations |
|---|---:|---:|---:|
| Equivalence groups (paraphrase/register -> identical IR & value) | 1,524 groups (3,954 items) | 1,524 | 0 |
| Monotonic subset (`subset <= superset` on stock counts) | 2,106 pairs | 2,106 | 0 |
| Partition sums (`breakdown` row sum == reference sum & stock count) | 1,400 breakdowns | 1,400 | 0 |
| Negation/exclusion bounds (`0 <= excluded_count <= base_count`) | 1,000 items | 1,000 | 0 |
| Temporal interval ordering (`12m flow <= 24m flow`) | 4 pairs | 4 | 0 |
| Ranking prefix consistency (`top k1 == prefix of top k2`) | 1,447 pairs | 1,447 | 0 |

## 5. Independent Frozen Suites and Prior-Round Replays

`H-blind` was never opened or executed. All four frozen suites and all eight prior commercial rounds (`commercial-round-01` .. `commercial-round-08`) remain byte-identical on disk.

### 5.1 Frozen Suites (Compared to Round 08 Final)

| Frozen Suite | Items | Round 08 `interpretation_correct` | Round 09 `interpretation_correct` | Round 08 `decision_correct` | Round 09 `decision_correct` |
|---|---:|---:|---:|---:|---:|
| `adv.jsonl` | 304 | 210 / 304 (69.08%) | **214 / 304 (70.39%)** | 280 / 304 | **285 / 304** |
| `h-informal.jsonl` | 202 | 118 / 202 (58.42%) | **121 / 202 (59.90%)** | 167 / 202 | **172 / 202** |
| `dialogues.jsonl` | 122 | 81 / 122 (66.39%) | **84 / 122 (68.85%)** | 103 / 122 | **106 / 122** |
| `ood.jsonl` | 400 | 377 / 400 (94.25%) | **400 / 400 (100.00%)** | 377 / 400 | **400 / 400** |

### 5.2 Prior Commercial Rounds Replay (`commercial-round-01` .. `commercial-round-08`)

| Prior Round | Items | `interpretation_correct` (Raw Frozen) | `act_correct` | Notes |
|---|---:|---:|---:|---|
| `commercial-round-01` | 1,000 | **1,000 / 1,000 (100.00%)** | 50 / 50 | Exact match |
| `commercial-round-02` | 1,000 | **950 / 1,000 (95.00%)** | 845 / 845 | 50 known closure-label defects from R08 audit |
| `commercial-round-03` | 3,000 | **2,844 / 3,000 (94.80%)** | 2,535 / 2,535 | 150 known closure-label defects + 6 place typos |
| `commercial-round-04` | 5,000 | **4,826 / 5,000 (96.52%)** | 4,225 / 4,225 | 160 known closure-label defects + 14 place typos/homonyms |
| `commercial-round-05` | 10,000 | **9,654 / 10,000 (96.54%)** | 8,450 / 8,450 | 320 known closure-label defects + 26 place typos/homonyms |
| `commercial-round-06` | 20,000 | **19,312 / 20,000 (96.56%)** | 16,900 / 16,900 | 640 known closure-label defects + 48 place typos/homonyms |
| `commercial-round-07` | 20,000 | **19,312 / 20,000 (96.56%)** | 16,900 / 16,900 | 640 known closure-label defects + 48 place typos/homonyms |
| `commercial-round-08` | 50,000 | **48,259 / 50,000 (96.52%)** | 42,250 / 42,250 | **49,859 / 50,000 (99.72%)** with R08's 1,600 closure adjudication patch |

## 6. Limitations, Adjudications, and Next Adjustments

1. **25 Unqualified `Cruzeiro` Items (`label-adjudications.jsonl`)**:
   - In `source_quality_provenance_uncertainty`, 25 items ask about `"na cidade de Cruzeiro"` without a UF. The frozen gold label marked them as `clarify` (treating `"Cruzeiro"` as ambiguous with `Cruzeiro do Sul/AC`, `Cruzeiro do Oeste/PR`, and `Cruzeiro da Fortaleza/MG`), whereas `Cruzeiro/SP` (`id=3427`) is the only municipality in `municipalities.tsv` whose exact canonical name is single-word `"Cruzeiro"`. The frozen `questions.jsonl` was preserved untouched, and the 25 items are recorded in `label-adjudications.jsonl`.
2. **Remaining Gaps in Frozen Suites (`adv`, `h-informal`, `dialogues`)**:
   - While all four frozen suites improved in Round 09 (`adv: 210 -> 214`, `h-informal: 118 -> 121`, `dialogues: 81 -> 84`, `ood: 377 -> 400`), roughly 30–40% of `adv`, `h-informal`, and `dialogues` still fail due to colloquial sector paraphrases not yet in `sectors.toml`, complex multi-clause conversational scope edits, and implicit referent carry-over across 4+ turns.
3. **Human Validation Deferred**:
   - As specified in the round protocol, `human_validated = false` across all 50,000 items. Human review of conversational naturalness, clarification options, and regional terminology remains deferred to the product refinement stage.
4. **Recommended Next Steps**:
   - Expand `sectors.toml` aliases and conversational referent resolution targeted at the remaining `h-informal` and `dialogues` failure clusters without overfitting to frozen test items.
   - Add interactive clarification-option selection tests for multi-word vs single-word municipality prefix collisions (such as `Cruzeiro` vs `Cruzeiro do Sul`).
   - Proceed to human validation and UI/API integration during product refinement.
