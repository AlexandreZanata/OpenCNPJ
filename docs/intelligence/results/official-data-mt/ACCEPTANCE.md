# First MT release acceptance corpus

Executed 2026-11-08 against the local `.ocind` directory
`/data/opencnpj-data/intelligence/official` and the built
`opencnpj-intel` binary.

- Cases: 261
- Passed: 261
- Failed: 0
- Corpus: `acceptance/cases.json`
- Machine-readable outcomes: `acceptance/results.json`
- Runner: `scripts/official-data/run_acceptance.py`

## Distribution

| Dimension | Count |
|---|---:|
| Positive `ok` | 252 |
| Empty selection (distinct from zero) | 3 |
| No observed value (missing denominator) | 1 |
| Clarification required | 1 |
| Unavailable source/capability | 3 |
| Unsupported request | 1 |
| Lookup | 197 |
| Trend | 18 |
| Compare | 16 |
| Ratio | 13 |
| Rank | 12 |
| Ask (refusals/clarifications) | 5 |

The corpus covers population, Census households/residents, municipal GDP,
CEMPRE, PAM temporary/permanent crops, PPM livestock, LSPA monthly state
production, RAIS annual employment, Senatran fleet and PIB/population ratios.
It includes temporal cases (`2020`, `2021`, `2023`, `2024`, `202601`-`202609`),
missing 2023 population, missing 2023 ratio denominator, a future 2024 GDP
request, a municipality without a 2022 Census cell, and explicit refusals for
company headcount, neighbourhood indicators and household income.

## Independent checks

Ten cases pin an independently extracted official value, for example:

- Cuiabá population 2024 = 682932
- Sorriso population 2024 = 120985
- Sorriso GDP 2023 = 13748757 (Mil Reais)
- Sorriso soybean production 2024 = 2084520 tonnes
- Sorriso bovine herd 2024 = 99790 head
- Mato Grosso soybean production 202609 = 50665337 tonnes
- Sorriso RAIS formal jobs 2024 = 39482
- Sorriso registered fleet 202609 = 112951
- Sorriso trucks 202609 = 3794
- Sorriso 2021 PIB/population = 12522102 / 94941

These values and their raw-evidence hashes are in `REAL-DATA.md`. The remaining
cases assert status, unit, period and source-aware execution rather than a copied
constant.

## Boundaries

- This is not human product validation.
- No target-hardware performance is claimed.
- The H-blind set was not opened.
- `Ask` coverage is a curated semantic set, not a claim that every Portuguese
  phrasing is understood.
- Unsupported versus incorrectly interpreted requests are reported separately;
  the corpus has one intentional unsupported case and no incorrect successful
  interpretation.
