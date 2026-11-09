# Stage 2 - essential IBGE context for MT

Status: implemented for the admitted IBGE aggregates; one source gap recorded.

## Queryable slices

| Registry dataset | Aggregate | Curated variables | Imported periods |
|---|---|---|---|
| `ibge-6579` | population estimate | `populacao_estimada` | 2001-2026 |
| `ibge-9922` | 2022 Census households/residents | `domicilios_ocupados`, `moradores_domicilios`, `media_moradores` | 2022 |
| `ibge-5938` | municipal GDP | `pib_municipal`, `va_agropecuaria`, `va_industria`, `va_servicos` | 2002-2023 |
| `ibge-9509` | CEMPRE | local units, organizations, employment, payroll, average wage | 2022-2024 |

The municipality catalogue already provided the IBGE/RFB bridge and population
estimates. The new stores are separate from `.ocnpj`/`.ocagg` and do not alter
their format.

## Operations

`lookup`, `compare`, `rank`, `trend` and an explicit `ratio`. Every answer carries
the reference period, unit, coverage, source dataset, licence status and
limitations. Missing, suppressed, not-available and observed-zero cells remain
distinct.

The default ratio period is the latest period present in both sources. For
PIB/population this was 2021 in the checked import; 2023 GDP has no 2023
population-estimate cell in aggregate 6579. A pinned 2023 ratio reports
`missing_denominator` instead of mixing years.

## Portuguese interpretation

`opencnpj-intel official ask` resolves:

- population, households, residents, GDP, value added and CEMPRE variables;
- municipality names and variants through the versioned municipality catalogue;
- MT state scope and comparisons;
- ranking, trend and explicit PIB-per-capita ratio;
- unavailable company headcount and neighbourhood/sector indicators as
  explicit refusals.

## Recorded gaps

- Municipal household income from the 2022 Census was not found in an admitted
  municipal API resource. It is not replaced with GDP, payroll or wage averages.
- The IBGE terms page returned HTTP 403 to the automated check. Reuse remains
  `review_required`; this is not a commercial-enablement decision.
- Number of sector establishments per 10,000 inhabitants is not implemented in
  this stage because it requires an explicit cross-source operation, not a
  concatenation of municipal and CNPJ answers.

Evidence: `REAL-DATA.md`, `logs/stage-2-*.log`.
