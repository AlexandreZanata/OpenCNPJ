# Inconsistencies and normalization backlog

Generated from `audit/cnpj-crosswalk-audit.json` and `audit/source-audit.json`.

## CNPJ-base findings

- 0 CNPJ municipality IDs outside the IBGE catalogue.
- 0 missing-municipality sentinel cells in T1/T2 marginals.
- 0 duplicate folded municipality names inside the same UF.
- 0 IBGE municipalities with zero active CNPJ establishments in T1.
- 0 official N6 geography codes without an IBGE catalogue match.
- 0 source-name mismatches after normalizing trailing `- UF` suffixes.

## Source/coverage findings

| Finding | Severity | Proposed resolution |
|---|---|---|
| Boa Esperança do Norte has no 2022 Census cell | info | Keep missing; population estimate is not a Census substitute |
| Boa Esperança do Norte absent from CEMPRE 2023 and PIB 2023 | info | Record coverage gap; refresh when IBGE publishes |
| PAM coffee missing Boa Esperança do Norte and Campos de Júlio | info | Observed absence, not a join failure |
| LSPA municipal detail not released | info | Keep state-level scope; no synthetic municipal series |
| 16 Senatran vehicle types use generated IDs | medium | Curate labels/synonyms or label as generated |
| Senatran `VILA BELA DA SANTISSIMA TRINDA` spelling | low | Keep explicit alias; already applied |
| PGFN malformed resource URLs | medium | Fix in adapter; reject malformed links |
| ANP maintenance response (503) observed | medium | Record availability per run |
| CGU/Compras/CNES/Comex/MT links not statically discoverable | high | Inspect each resource/API before admission |
| ANTT JSON larger than probe limit | low | Stream with a bounded adapter and inspect schema |

## Resolved during this pass

- RAIS six-digit to seven-digit IBGE municipality bridge: all MT rows matched.
- IBGE `- UF` name suffixes are normalized only for comparison; identity remains
  the IBGE code.
- Senatran municipality names are bridged through the catalogue with an explicit
  alias for the one known spelling difference.
