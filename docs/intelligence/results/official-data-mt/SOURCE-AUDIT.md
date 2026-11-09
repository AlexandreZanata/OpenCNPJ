# Additional source verification and raw-data categorization

Executed 2026-11-08 against the live source registry. The audit downloads a
bounded sample of each resource exactly as received, records HTTP status,
content type, bytes, SHA-256, detected format and a first-pass schema, and never
normalizes the raw bytes in place.

## Scope

- 15 registered sources and 10 admitted datasets.
- 24 probes: 8 JSON, 3 XLSX, 1 semicolon CSV, 11 HTML pages and 1 truncated JSON
  sample.
- 0 network errors. ANP returned HTTP 503 "Em manutenção" in the first pass and
  HTTP 200 in a later pass, so availability is recorded as variable.
- Large national resources were sampled, not committed. Full RAIS and Senatran
  XLSX files were downloaded because they are small enough; the ANTT JSON and
  Comex Stat page hit the configured probe limit and are marked `truncated`.

Machine-readable result: `audit/source-audit.json`. Raw samples:
`/data/opencnpj-data/intelligence/official/audit/raw/`.

## Categorization

| Category | Probes |
|---|---:|
| agriculture | 2 |
| census | 1 |
| compliance | 1 |
| economy | 2 |
| education | 1 |
| energy | 2 |
| fiscal | 2 |
| fuel | 1 |
| health | 1 |
| labour | 2 |
| local_government | 1 |
| macroeconomy | 1 |
| population | 1 |
| procurement | 2 |
| trade | 1 |
| transport | 3 |

| Source | Resource checked | Format | Result |
|---|---|---|---|
| IBGE | metadata for aggregates 6579, 5938, 9509, 1612, 6588, 9922 | JSON | HTTP 200, object with variables/classifications/periodicity |
| MTE | RAIS 2024 tables 2023x2024 | XLSX | HTTP 200, full 2.54 MB, 9 worksheets |
| MTE | Novo Caged page | HTML | HTTP 200, no stable aggregate contract |
| Senatran | fleet by municipality/type, 202609 | XLSX | HTTP 200, full 0.74 MB, 22 vehicle columns |
| ANTT | RNTRC dataset page | HTML | HTTP 200; CSV and JSON resources linked |
| ANTT | RNTRC vehicles JSON | JSON | HTTP 200; >1 MB sample truncated, schema not admitted |
| PNCP | contracts 202609, modality 6 | JSON | HTTP 200, 849 KB, keys `data`, `totalPaginas`, `totalRegistros` |
| Compras | Swagger UI | HTML | HTTP 200, API contract not admitted |
| CGU | CEIS download page | HTML | HTTP 200, no direct static link discovered |
| CNES | dataset page | HTML | HTTP 200, only policy PDFs discovered |
| INEP | Censo Escolar page | HTML | HTTP 200, 31 ZIP microdata links discovered |
| ANP | price survey page | HTML | HTTP 503 then 200; variable availability |
| Comex Stat | home page | HTML | HTTP 200, page truncated at probe limit |
| BCB | SGS series 1, last 5 | JSON | HTTP 200, array of `data`/`valor` |
| ANEEL | GD wind-technical CSV | CSV `;` | HTTP 200, 8 columns, ODbL declared |
| PGFN | field dictionary | XLSX | HTTP 200, workbook present |
| PGFN | active-debt page | HTML | HTTP 200; malformed resource links found |
| MT | open-data portal | HTML | HTTP 200, no direct dataset link discovered |

## CNPJ-base crosswalk

Report: `audit/cnpj-crosswalk-audit.json`.

| Check | Result |
|---|---|
| CNPJ municipality IDs absent from the catalogue | 0 |
| Missing-municipality sentinel cells | 0 |
| Duplicate folded municipality names in the same UF | 0 |
| IBGE municipalities with zero active CNPJ establishments | 0 |
| Official N6 geography codes absent from the IBGE catalogue | 0 |
| Official source name mismatches after normalizing `- UF` suffixes | 0 |

The RAIS six-digit codes were bridged to seven-digit IBGE codes; all 142 MT
municipalities matched. Senatran has one source spelling difference
(`VILA BELA DA SANTISSIMA TRINDA`) handled by an explicit alias, after which all
142 MT municipalities matched. IBGE API names carry a ` - MT` suffix; the audit
normalizes that suffix for comparison and keeps the IBGE code as identity.

## Normalization backlog

| Item | Severity | Action |
|---|---|---|
| Boa Esperança do Norte (IBGE 5101837) has no 2022 Census cell | info | Keep absent; do not backfill from the population estimate |
| Boa Esperança do Norte missing from CEMPRE 2023 and PIB 2023 | info | Record source coverage gap; refresh when published |
| Coffee (PAM 1613) has no cells for Boa Esperança do Norte or Campos de Júlio | info | Treat as observed absence, not a failed join |
| LSPA is state-level only | info | Keep N3 scope explicit; never fabricate municipal monthly data |
| 16 Senatran vehicle types are generated IDs, not curated variables | medium | Add curated labels/synonyms or mark as generated-only |
| Senatran spelling alias for Vila Bela da Santíssima Trindade | low | Keep in a versioned source alias table |
| PGFN page has malformed links (`http://http://...`, relative `http://Dados...`) | medium | Repair at the source adapter; do not follow blindly |
| ANP availability changed between passes (503 then 200) | medium | Record availability per ingestion, not as a permanent state |
| CGU/Compras/CNES/Comex/MT data links are JS/API-driven or absent | high | Inspect resource APIs individually before admission |
| ANTT JSON exceeds the probe limit | low | Download with a dedicated bounded streaming adapter before schema admission |

No inconsistency was found between CNPJ municipality identifiers and the
municipality catalogue. The remaining items are source coverage, naming,
availability or uncurated-field issues to resolve before admitting more data.
