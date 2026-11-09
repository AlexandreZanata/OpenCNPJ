# Stage 5 - fleet and transport

## Implemented: Senatran municipal fleet

The official September 2026 workbook
`Frota_por_municipio_e_tipo_Setembro_2026.xlsx` is imported for MT. It contains
the registered fleet by municipality and 22 vehicle types, including total,
trucks, pickup trucks, motorcycles, cars and buses.

- The source has UF + municipality name, not an IBGE code. Names are bridged
  through the versioned municipality catalogue; ambiguous or unmatched rows are
  counted and reported.
- One explicit alias is recorded for the source spelling
  `VILA BELA DA SANTISSIMA TRINDA`; after the alias all 142 MT municipalities
  matched.
- Registered fleet is not circulating fleet. A municipal total does not identify
  a CNPJ's vehicles, and publication is not a real-time feed.

## Not admitted: ANTT RNTRC

Aggregate history and identified resources have different schemas, coverage and
permitted uses. No CNPJ/fleet ownership link and no service-quality inference is
claimed. Schema inspection remains an external follow-up.

Vehicles per related establishment is not implemented: it requires an explicit
CNPJ-aggregate cross-source operation, not an inferred join.

Evidence: `REAL-DATA.md`, `logs/stage-5-*`.
