# Stage 3 - labour context

## Implemented: RAIS 2024 structural employment

The official annual workbook `RAIS Complementar - Tabelas 2023x2024.xlsx` is
imported from sheet `TABELA 4` (formal jobs by UF, municipality and activity
group). The MT slice covers 142 municipalities, 2023 and 2024, and six groups:
agriculture, industry, construction, commerce, services and total.

- Six-digit RAIS codes are bridged to seven-digit IBGE codes through the
  versioned municipality catalogue.
- Unmatched rows are counted and reported; they are never silently dropped.
- The unit is `Vínculos`; this is an annual administrative stock, not a monthly
  flow and not an individual CNPJ's headcount.
- There is no derivation of a stock from admissions minus separations.

## Blocked: Novo Caged monthly aggregates

No small verified municipal aggregate endpoint was admitted. The official
microdata are published on the PDET FTP server; monthly table workbooks are
linked through a Google Drive folder. Downloading and processing national
microdata is outside this slice, and a Google Drive folder is not a stable bulk
API contract. No CAGED numbers were invented.

Exact dependency: a stable, documented, machine-readable Novo Caged aggregate
with municipal/sector dimensions and revision metadata.

Evidence: `REAL-DATA.md`, `logs/stage-3-*`.
