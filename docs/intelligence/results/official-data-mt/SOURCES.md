# Official source registry and `.ocind` lifecycle

Checked 2026-11-08. The machine-readable registry is
`crates/opencnpj-insights/official/sources.toml`; it is compiled into
`opencnpj-insights` and validated on load.

## Admitted states

`discovered`, `verified`, `fetched`, `normalized`, `queryable`,
`commercially_enabled`, `stale`, `failed`, `unavailable`.

`state` is separate from `licence_status`. A source may be technically verified
while its reuse terms are still `review_required`. No source is marked
`commercially_enabled` by this slice, and no blanket CC BY licence is assumed.

## Registry snapshot

| Source | Verified state | Resource-level licence | First admitted use |
|---|---|---|---|
| IBGE | verified | review_required | population, Census, GDP, CEMPRE, PAM, PPM, LSPA |
| MTE | discovered | review_required | no verified small bulk aggregate admitted yet |
| Senatran | verified | review_required | monthly XLSX fleet files linked by the official page |
| ANTT | discovered | verified (CC Attribution for checked resource) | aggregate/identified schemas not admitted yet |
| PNCP | discovered | review_required | institutional page returned HTTP 401 to automated request |
| Compras.gov.br | verified | review_required | Swagger UI reachable |
| CGU | verified | review_required | CEIS/CNEP download page reachable |
| CNES | discovered | review_required | not admitted |
| INEP | discovered | review_required | not admitted |
| ANP | discovered | review_required | not admitted |
| Comex Stat | verified | review_required | not admitted |
| BCB | verified | review_required | not admitted |
| ANEEL | discovered | verified ODbL | not admitted; share-alike assessment pending |
| PGFN | discovered | review_required | not admitted |
| Dados Abertos MT | discovered | review_required | no individually verified dataset admitted yet |

The IBGE metadata API returned HTTP 200. The IBGE terms page returned HTTP 403
to the automated check; this is recorded as `review_required`, not as a licence
grant. The PNCP institutional page returned HTTP 401; the API contract was not
admitted.

## Published indicator assets

An imported indicator asset is an immutable `.ocind` SQLite file with one embedded
manifest. It stores:

- source, dataset, publisher, API and metadata URLs;
- licence, licence URL, licence status and attribution;
- retrieval time and optional publication time;
- coverage, reference periods, unit and limitations;
- raw-response SHA-256 and byte count;
- total, accepted, rejected, duplicate and malformed row counts;
- normalized observations with explicit cell state: `observed`, `missing`,
  `suppressed`, `not_available` or `malformed`.

The writer creates a temporary file, verifies SQLite integrity and row
accounting, then publishes with no-clobber semantics. A failed refresh cannot
overwrite a serving artifact.

## Commands

```bash
opencnpj-intel sources list
opencnpj-intel sources list --json
opencnpj-intel sources status --dir /data/opencnpj-data/intelligence/official
opencnpj-intel sources verify --dir /data/opencnpj-data/intelligence/official
```

`status` merges the compiled registry with locally published manifests. A
dataset with an imported manifest becomes `queryable`; an absent dataset keeps
its registry state. `verify` checks SQLite integrity, the embedded manifest,
row accounting and prints the asset SHA-256.
