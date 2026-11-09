# Stages 7-9 - revalidated sources and blockers

No procurement, additional-vertical or licence-sensitive dataset was integrated in
this slice. The endpoints below were rechecked on 2026-11-08. A check is not an
admission and no simulated production data is substituted.

## Stage 7 - government sales

| Resource | Check | Decision |
|---|---|---|
| PNCP contracts API | `GET https://pncp.gov.br/api/consulta/v1/contratos?...` returned HTTP 200 with JSON | Not admitted: contract-level identifiers, pagination, cross-portal deduplication and amount semantics need a separate store; the public HTML page returned HTTP 401 to the automated check |
| Compras.gov.br | Swagger UI HTTP 200; the guessed `/modulo-contratos/v1/contratos` path returned HTTP 404 | Not admitted: contract not verified |
| CGU CEIS/CNEP | Download page HTTP 200 | Not admitted: sanction scope, effective dates and "absence is not a certificate" rules need a dedicated adapter |

Boundaries preserved: estimated, awarded, contracted and paid amounts are
distinct; procurement is not private B2B demand; no sanction record is a
universal certificate of good standing.

## Stage 8 - additional verticals

| Resource | Check | Decision |
|---|---|---|
| CNES | dataset page HTTP 200 | Not admitted; CNES IDs are not CNPJ and personal-professional fields stay out |
| INEP Censo Escolar | page HTTP 200 | Not admitted; school IDs are not CNPJ |
| ANP fuel prices | page HTTP 200 | Not admitted; the survey is a sample, not all stations or a live feed |
| Comex Stat | page HTTP 200 | Not admitted; municipal detail uses fiscal domicile and SH4, and does not support CNPJ exporter lists |
| BCB | page HTTP 200 | Not admitted; series-specific units/frequencies/revisions need curation |
| Dados Abertos MT | portal HTTP 200 | No individually verified dataset admitted |

## Stage 9 - licence-sensitive energy and fiscal context

| Resource | Check | Decision |
|---|---|---|
| ANEEL distributed generation | dataset page HTTP 200; ODbL declared | Not admitted; adapted-database attribution/share-alike assessment is pending |
| PGFN active debt | page HTTP 200 | Not admitted; quarterly federal/FGTS scope, debt-state semantics and personal-data exclusion must be implemented before use |

Commercial enablement for IBGE, MTE and Senatran also remains pending because
their resource-level terms are recorded as `review_required`. A public endpoint
returning HTTP 200 is not a licence grant.
