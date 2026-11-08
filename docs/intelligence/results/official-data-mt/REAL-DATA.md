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
