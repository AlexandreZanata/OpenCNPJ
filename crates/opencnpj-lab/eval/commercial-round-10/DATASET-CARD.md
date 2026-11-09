# Dataset Card — Commercial Round 10 (`commercial-round-10`)

## Overview

- **Total items**: `50,000` pt-BR commercial questions and dialogue turns (`batch-001` 1,000-item pilot + `batch-002..batch-099` 98 batches of 500 items).
- **Questions JSONL SHA-256**: `666c6ed11915b128fd02749f228061a8036e4d4d7d66a2e1a21a9ab06d703460`
- **Questions TSV SHA-256**: `7fcc68e75689946645b16f452a60d3f1ffdd2288ae1bbaed653b631c85c28fc9`
- **Data anchor**: `2026-10-11` (`Day(46305)`), current competence `2026-10` (`Competence(321)`).
- **Human validation**: `false` (deferred to product refinement).

## Quotas & Distributions

- **Geography (`50,000`)**: MT `30,000` across six strata covering every catalogued MT municipality; other 26 UFs `12,000` (floor 400 per UF); national/cross-UF/comparative `8,000`.
- **Primary business families (`50,000`)**: market size `5,000`; prospecting `7,000`; historical `5,000`; geography `5,000`; corporate `4,000`; supply chain `5,000`; cross-variable `5,000`; dialogue `6,000`; source quality `3,000`; unsupported `3,000`; tax/numeric `2,000`.
- **Registers (`50,000`)**: `conversational_professional` `35,000`; `formal_technical` `10,000`; `noisy_short` `5,000`.
- **Splits (`50,000`)**: `synthetic_dev` `35,000`; `synthetic_eval` `10,000`; `synthetic_challenge` `5,000`, partitioned by scenario/skeleton/session/CNPJ root.
- **Overlapping targets**: MT-context `34250`; other-UF regional `5624`; novel semantic constructions `16831`; clarify+reject `8000`.
- **Novelty tags**: `{"None": 33169, "colloquial_sector:posto": 127, "colloquial_sector:pet": 122, "colloquial_sector:abatedouro": 128, "colloquial_sector:eletricista": 118, "colloquial_sector:academia": 125, "opened_to_closure_cohort_boundary": 1000, "state_internal_exclusion": 1000, "colloquial_sector:funilaria": 120, "supply:gráfica+serviço de fretamento de ônibus": 19, "cnae_section_division_breakdown": 1000, "supply:oficina+loja de artesanato": 10, "supply:funilaria+corretor de imóveis": 20, "population_band_density_rank": 1000, "sector_by_tax_regime_share": 1000, "multi_turn_compound_scope_edit": 6000, "mt_and_national_homonym_municipality_guard": 500, "regional_or_agro_ambiguous_activity_clarify": 500, "temporal_validation_guard": 500, "mt_informal_subregion_clarification": 500, "municipal_prefix_exact_match": 2390, "colloquial_sector:pizzaria": 128, "colloquial_sector:gráfica": 121, "supply:loja de doces+gráfica": 21, "supply:eletricista+empresa de transporte de passageiros": 19, "colloquial_sector:oficina": 118, "supply:posto+serralheria": 19, "supply:academia+transportadora rodoviária de cargas": 18, "supply:pet+consultório de psicologia": 19, "supply:atacado de frutas e verduras+funilaria": 19, "supply:agência de recrutamento+posto": 17, "supply:pizzaria+loja de cama mesa e banho": 18, "supply:ateliê de costura sob medida+oficina": 9, "supply:loja de brinquedos+eletricista": 18, "supply:transportadora rodoviária de cargas+academia": 18, "supply:clínica veterinária+pet": 18, "supply:empresa de coleta de resíduos comuns+pizzaria": 18, "supply:empresa de transporte de passageiros+frigorífico": 10, "supply:abatedouro+assistência técnica de eletrodomésticos": 9, "supply:ateliê de costura sob medida+mecânica": 9, "supply:empresa de transporte de passageiros+abatedouro": 8, "supply:frigorífico+assistência técnica de eletrodomésticos": 9, "supply:mecânica+loja de artesanato": 9}`.
