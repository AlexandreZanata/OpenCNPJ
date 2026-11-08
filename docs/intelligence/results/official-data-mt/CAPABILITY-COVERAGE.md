# Capability coverage

| Source | Dataset | Operations | pt-BR examples | Acceptance cases |
|---|---|---|---|---|
| IBGE | `ibge-6579` population estimate | lookup, compare, rank, trend, ratio | "Qual a população estimada de Sorriso em 2024?" / "Ranking dos municípios de MT por população em 2024" | `pop-*`, `gdp-per-capita-*`, `ask-pop-*`, `ask-rank-pop-mt`, `ask-trend-pop-sorriso` |
| IBGE | `ibge-9922` Census 2022 households | lookup, compare, trend | "Quantos domicílios ocupados há em Sorriso?" | `census-*`, `census-missing-boa-esperanca` |
| IBGE | `ibge-5938` municipal GDP | lookup, rank, trend, ratio | "PIB de Sorriso em 2023" / "PIB per capita de Sorriso" | `gdp-*`, `ask-pib-sorriso`, `ask-pib-per-capita` |
| IBGE | `ibge-9509` CEMPRE | lookup, rank | "Unidades locais em Sorriso" | `cempre-*` |
| IBGE | `ibge-1612` PAM temporary crops | lookup, rank, trend | "Produção de soja em Sorriso em 2024" | `pam-*`, `ask-soy-*` |
| IBGE | `ibge-1613` PAM permanent crops | lookup | "Produção de café em Sorriso" | `pam-coffee-*` |
| IBGE | `ibge-3939` PPM livestock | lookup, rank | "Rebanho bovino de Sorriso em 2024" | `ppm-*`, `ask-cattle` |
| IBGE | `ibge-6588` LSPA | lookup, trend | "Produção de soja no Mato Grosso em 202609" | `lspa-*` |
| MTE | `mte-rais-2024` RAIS | lookup, rank | "Empregos formais em Sorriso em 2024" | `rais-*`, `ask-rais` |
| Senatran | `senatran-frota-2026-09` fleet | lookup, compare, rank | "Frota total de Sorriso em setembro de 2026" | `fleet-*`, `ask-fleet` |

## Explicit refusals / boundaries in the corpus

| Request | Status |
|---|---|
| Company headcount by CNPJ | `unavailable` |
| Neighbourhood/sector indicator | `unavailable` |
| Household income | `unavailable` |
| Crop without a product | `clarification_required` |
| Unsupported attribute | `unsupported` |
| Missing 2023 population | `empty` |
| Missing 2023 ratio denominator | `no_observed_value` |
| Future 2024 GDP | `empty` |
| Census cell not released for Boa Esperança do Norte | `empty` |

## Record-level adapters (`.ocrec`)

| Source | Dataset | Adapter | Full national records |
|---|---|---|---:|
| ANTT | `antt-rntrc-veiculos` | JSON streaming/buffer | 98,119 |
| CNES | `cnes-estabelecimentos` | CSV/ZIP | 641,928 accepted (340 rejected) |
| INEP | `inep-censo-escolar-2024` | CSV/ZIP | 215,545 |
| PGFN | `pgfn-fgts-2026Q3` | CSV/ZIP | 549,458 accepted |
| PGFN | `pgfn-previdenciario-2026Q3` | CSV/ZIP | 3,287,586 accepted |

## Not implemented / blocked

- Novo Caged monthly aggregate.
- ANTT RNTRC.
- PNCP/Compras/CGU procurement pack.
- CNES, INEP, ANP, Comex Stat, BCB, ANEEL, PGFN and MT portal verticals.
- Cross-source CNPJ-establishment ratios and correlation coefficients.
