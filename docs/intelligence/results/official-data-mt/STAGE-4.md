# Stage 4 - MT agriculture

## Implemented

- `ibge-1612` temporary crops: area planted, area harvested, production, yield
  and production value for the curated products soybean, maize, cotton, rice,
  beans and sugar cane, 2020-2025.
- `ibge-1613` permanent crops: coffee, 2020-2025, using source variable `2313`
  for the area destined for harvest (the API rejected variable `109` for coffee).
- `ibge-3939` livestock: bovine, swine, poultry and sheep, 2020-2025.
- `ibge-6588` LSPA: monthly soybean area/production for Mato Grosso state,
  the geography actually released by the resource.

## Boundaries

- Hectares, tonnes, kilograms per hectare and mil reais remain distinct units.
- Product categories are explicit. Coffee total and variety categories are not
  summed blindly.
- Annual municipal PAM production is not divided into a synthetic monthly
  municipal series.
- A regional production total does not identify a farm, a CNPJ's land,
  machinery, herd or purchasing intent.
- Comparing agriculture with related-CNAE establishment counts is not implemented;
  it needs an explicit cross-source operation, not a concatenated answer.

Evidence: `REAL-DATA.md`, `logs/stage-2-real-queries.log`.
