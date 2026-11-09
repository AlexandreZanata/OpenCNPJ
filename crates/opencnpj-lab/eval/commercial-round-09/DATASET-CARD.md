# Dataset Card — Commercial Round 09 (`commercial-round-09`)

## Overview

- **Total items**: `50,000` pt-BR commercial questions and dialogue turns (`batch-001` 1,000-item pilot + `batch-002..batch-099` 98 batches of 500 items).
- **Questions JSONL SHA-256**: `a3bc6c78b3066f4f8721b2de30948a04278adc80ca4cbd292acfb978eb763c29`
- **Questions TSV SHA-256**: `29fd87e90d9148d79723a56ac45ad902c682c937d11ed720adf593be7eec6751`
- **Data anchor**: `2026-10-11` (`Day(46305)`), current competence `2026-10` (`Competence(321)`).
- **Human validation**: `false` (deferred to product refinement per `ROUND-09-EXPANSION.md`).

## Quotas & Distributions

- **Geography (`50,000`)**:
  - Mato Grosso (`30,000` across 6 strata covering all `142/142` MT municipalities): `mt_metropolitan` (`5,000`), `mt_north_agro_industrial` (`6,000`), `mt_south_logistics_industrial` (`5,000`), `mt_east_araguaia_xingu` (`4,500`), `mt_west_parecis_guapore` (`4,500`), `mt_pantanal` (`5,000`).
  - Other 26 UFs (`12,000`, minimum `>= 400` per UF).
  - National, cross-UF, macro-regional, comparative, and unresolved-homonym (`8,000`).
- **Primary Business Families (`50,000`)**:
  - `market_size_density_competition`: `5,000`
  - `prospecting_qualification_territory`: `7,000`
  - `historical_stock_flows_seasonality`: `5,000`
  - `geography_catchments_comparisons`: `5,000`
  - `corporate_structure_hq_branches_identity`: `4,000`
  - `supply_chain_complementary`: `5,000`
  - `cross_variable_correlation`: `5,000`
  - `dialogue_corrections_references_planning`: `6,000` (`2,000` complete 3-turn dialogues)
  - `source_quality_provenance_uncertainty`: `3,000`
  - `unsupported_missing_contradictions`: `3,000`
  - `tax_size_legal_capital_numeric`: `2,000`
- **Registers (`50,000`)**: `conversational_professional` (`35,000`), `formal_technical` (`10,000`), `noisy_short` (`5,000`).
- **Splits (`50,000`)**: `synthetic_dev` (`35,000`), `synthetic_eval` (`10,000`), `synthetic_challenge` (`5,000`), strictly partitioned by `scenario_family_id`, `surface_skeleton_id`, CNPJ lookup roots, and `session_id`.
- **Overlapping Targets**:
  - MT-context regional/economic items: `35540` (`>= 8,000` target)
  - Other-UF regional items: `4834` (`>= 4,000` target)
  - Novel semantic constructions vs rounds 01–08: `41075` (`>= 15,000` target)
  - `clarify` + `reject` outcomes: `8500` (`4700` `clarify` + `3800` `reject`; `>= 5,000` target)
