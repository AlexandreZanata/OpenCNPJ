# Real-data reference checks (IBGE, MT)

Checked 2026-11-08 against the live IBGE aggregate API v3. Raw responses are
kept outside Git under `/data/opencnpj-data/intelligence/official/raw/`.
The values below were extracted independently from those raw JSON files with a
small Python reader and then compared with the `.ocind` query engine.

## Selected cells

| Dataset | Source variable / class / category | Geography | Period | Value | Unit |
|---|---|---|---|---:|---|
| `ibge-6579` | `9324` population estimate | Sorriso (5107925) | 2024 | 120985 | Pessoas |
| `ibge-5938` | `37` municipal GDP | Sorriso (5107925) | 2023 | 13748757 | Mil Reais |
| `ibge-9509` | `706` local units (CEMPRE) | Sorriso (5107925) | 2023 | 9251 | Unidades |
| `ibge-9922` | `382` residents in occupied households | Sorriso (5107925) | 2022 | 110143 | Pessoas |
| `ibge-1612` | `214` production, class `81`, category `2713` soybean | Sorriso (5107925) | 2024 | 2084520 | Toneladas |
| `ibge-3939` | `105` herd, class `79`, category `2670` bovine | Sorriso (5107925) | 2024 | 99790 | Cabeças |
| `ibge-6588` | `35` production, class `48`, category `39443` soybean | Mato Grosso (51) | 202609 | 50665337 | Toneladas |

`ibge-6588` is monthly and state-level only. The resource does not release
monthly municipal detail; no monthly municipal figure was derived.

## Reproducible extraction commands

```bash
# Raw API samples
curl -sS 'https://servicodados.ibge.gov.br/api/v3/agregados/6579/periodos/2024/variaveis/9324?localidades=N6[N3[51]]'
curl -sS 'https://servicodados.ibge.gov.br/api/v3/agregados/5938/periodos/2023/variaveis/37?localidades=N6[5107925]'
curl -sS 'https://servicodados.ibge.gov.br/api/v3/agregados/1612/periodos/2024/variaveis/214?localidades=N6[5107925]&classificacao=81[2713]'
curl -sS 'https://servicodados.ibge.gov.br/api/v3/agregados/6588/periodos/202609/variaveis/35?localidades=N3[51]&classificacao=48[39443]'
```

## Query checks executed

```bash
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation lookup --variable populacao_estimada --geography 5107925 --period 2024
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation ratio --variable pib_municipal --denominator populacao_estimada --geography 5107925
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation lookup --variable pam_producao --classification 81 --category 2713 \
  --geography 5107925 --period 2024
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation lookup --variable ppm_efetivo --classification 79 --category 2670 \
  --geography 5107925 --period 2024
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation trend --variable lspa_producao --classification 48 --category 39443 \
  --geography 51 --period 202601 --period 202605 --period 202609
```

The default PIB/population ratio selects the latest common observed period
(2021 in this import) rather than mixing 2023 GDP with an estimate from another
year. A request pinned to 2023 reports `missing_denominator`; it does not impute
or borrow another year.

## Raw evidence hashes

| Raw file | Bytes | SHA-256 |
|---|---:|---|
| `ibge-6579.raw.json` | 61636 | `d648c74d9153752ed1faf72398bb4b96cc0f2b53fa2d96ecc3ddb00facd44314` |
| `ibge-5938.raw.json` | 251537 | `f383c5d6efcce56f63c1fc7cb2b9f7a2ab8500f04e965f08f56591bbe0523701` |
| `ibge-9509.raw.json` | 130712 | `98227236b559fac5c5bd8b26d3ac5a41899d912ee7d2f5ca17fd69058e501661` |
| `ibge-9922.raw.json` | 53387 | `43d88e2711fb05f8af685df31e7e804cc7a6d70fa0d7921002446a0376bb3a67` |
| `ibge-1612.raw.json.5` (soybean) | 142037 | `755bfe9a7efe214f4c33c0b139a7b3291cdbd37fb699dc885ac07cc2c820b5c5` |
| `ibge-3939.raw.json` (bovine, first request) | 29239 | `1fb2f0771431694cc09579ab6faa22009e4bb417c72cd55678e67e67a6492acd` |
| `ibge-6588.raw.json` | 14223 | `06db7f7beb3d1b07be86dfec9b05eb04ea81c22daf845b9276c20a2781409fcd` |

## Published store hashes

| Store | Rows | SHA-256 |
|---|---:|---|
| `ibge-6579.ocind` | 3146 | `63d176628036c6312a052384d933b22f22192ac5c5390fdc7ba51b25403607a7` |
| `ibge-5938.ocind` | 12496 | `59331e4f04128b1b7326cae346f64e3e351f864b7ac0d5745023cb65f6bb82f9` |
| `ibge-9509.ocind` | 2538 | `61d2b28179b385eb63f2605cb3a1feeb3bf15ca6c97c9b8a7fc542c932317d27` |
| `ibge-9922.ocind` | 423 | `02fd9b5166057eeea83b05189808b1081b0ca5c4f141ad51fb9e24cb90098afc` |
| `ibge-1612.ocind` | 25560 | `c84ccb1bdd6460223b8eeae41ac6907ee596d499fc2c98a7d51e0bbc38b576c9` |
| `ibge-1613.ocind` | 4200 | `84f307874bc1d3765bb23be0a721993de2d316dd8de167c442d15772add704f2` |
| `ibge-3939.ocind` | 3408 | `752f9a21f795bc59611154bfbf0611121485ab9a889c9fbc75745dd945503542` |
| `ibge-6588.ocind` | 723 | `bb4ca2f93c41d379a308828c7d74c792ad8a04305fad1d797e5de064acb90527` |

## Boundaries

- No national full dataset was committed.
- The raw files and `.ocind` stores are local evidence, not portable Git assets.
- IBGE terms still require legal review for commercial enablement.
- Municipal income from the 2022 Census was not found in an admitted municipal
  API resource; it is not substituted with GDP, payroll or another unit.
- Municipal GDP per capita is not household income or purchasing power.

## Stage 3 - RAIS 2024

Source: official `RAIS Complementar - Tabelas 2023x2024.xlsx`, sheet `TABELA 4`.
The workbook uses six-digit municipality codes; the importer bridges them to the
seven-digit IBGE code through the versioned municipality catalogue. For the MT
slice, 142 municipalities matched.

| Dataset | Variable | Geography | Period | Value | Unit |
|---|---|---|---|---:|---|
| `mte-rais-2024` | `rais_emprego_total` | Sorriso (5107925) | 2023 | 38783 | Vínculos |
| `mte-rais-2024` | `rais_emprego_total` | Sorriso (5107925) | 2024 | 39482 | Vínculos |

Independent extraction from `C5324 = 510792`, `Y5324 = 38783`, `Z5324 = 39482`
in the official workbook. Store rows: 1704 (142 municipalities x 6 variables x
2 years). Raw SHA-256
`175c696341bb2821dc6a210367ccd20ba8e9e5f9d790daef195b76b3ee90ab38`; store
SHA-256 `b646cc727c9f97b9d415a464a8fd7afe1c69211e356488cdf62be4fc0009a8cb`.

Novo Caged monthly aggregates remain blocked: no small verified municipal
aggregate endpoint was admitted. The public microdata are on the PDET FTP server
and the monthly tables are distributed through a Google Drive folder. Neither is
used to fabricate a monthly series here.

## Stage 4 - PAM, PPM and LSPA

| Dataset | Source variable / class / category | Geography | Period | Value | Unit |
|---|---|---|---|---:|---|
| `ibge-1612` | area planted `109`, class `81`, soybean `2713` | Sorriso | 2024 | 600000 | Hectares |
| `ibge-1612` | production `214`, class `81`, soybean `2713` | Sorriso | 2024 | 2084520 | Toneladas |
| `ibge-1613` | production `214`, class `82`, coffee `2723` | MT municipalities | 2020-2025 | imported | Toneladas |
| `ibge-3939` | herd `105`, class `79`, bovine `2670` | Sorriso | 2024 | 99790 | Cabeças |
| `ibge-6588` | production `35`, class `48`, soybean `39443` | Mato Grosso (51) | 202609 | 50665337 | Toneladas |

`ibge-6588` is state-level for this resource. No monthly municipal production was
synthesized from the annual PAM figure. LSPA raw SHA-256
`06db7f7beb3d1b07be86dfec9b05eb04ea81c22daf845b9276c20a2781409fcd`; store
SHA-256 `bb4ca2f93c41d379a308828c7d74c792ad8a04305fad1d797e5de064acb90527`.

## Stage 5 - Senatran registered fleet, September 2026

| Dataset | Variable | Geography | Period | Value | Unit |
|---|---|---|---|---:|---|
| `senatran-frota-2026-09` | `frota_total` | Sorriso (5107925) | 202609 | 112951 | Veículos |
| `senatran-frota-2026-09` | `frota_caminhao` | Sorriso (5107925) | 202609 | 3794 | Veículos |
| `senatran-frota-2026-09` | `frota_caminhao` | Sinop (5107909) | 202609 | 5588 | Veículos |

The official sheet has UF + municipality name, not an IBGE code. The importer
bridges names to seven-digit IBGE codes through the versioned catalogue and
records unmatched rows. One known spelling difference
(`VILA BELA DA SANTISSIMA TRINDA`) is handled by an explicit alias with evidence;
after that, all 142 MT municipalities matched. Store rows: 3124 (142 x 22 types).
Raw SHA-256 `889d133da1ee2c301ad9a3e6b41b24fbb94286b4ba483c15cb706775bef5e16a`;
store SHA-256 `c3d9e04c44c466a07d222b158cced91eb7ae84cbf567c0f5f4c3c63f04d452b6`.

ANTT RNTRC remains not admitted: aggregate history and identified resources have
different schemas and coverage. No CNPJ/fleet ownership link is claimed.
