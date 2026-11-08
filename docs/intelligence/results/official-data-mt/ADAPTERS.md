# Full-resource streaming adapters

Implemented 2026-11-08 for the record-level sources named in the audit. Each
adapter reads the complete published resource, keeps source-specific columns as
JSON in an immutable `.ocrec` SQLite store, and accounts for every source row as
accepted, out of scope, rejected or duplicate.

## Adapters

| Adapter | Source | Input | Common keys | Default scope |
|---|---|---|---|---|
| `antt` | ANTT RNTRC | JSON (`veiculos` array) | UF, vehicle category, transport category | MT |
| `cnes` | CNES establishments | CSV inside ZIP | CNES/unit IDs, `CO_IBGE`, CNPJ fields | MT |
| `inep` | INEP Censo Escolar 2024 | CSV inside ZIP | school ID, `CO_MUNICIPIO`, CNPJ fields | MT |
| `pgfn` | PGFN active debt (FGTS/Previdenciário) | CSV inside ZIP | CNPJ, inscription, UF | MT, legal persons |
| `bcb` | BCB SGS series | JSON array | date, series value | all (national) |
| `aneel` | ANEEL distributed generation | CSV inside ZIP | `CodEmpreendimento`, `CodMunicipioIbge`, CNPJ | MT, legal persons |
| `anp` | ANP fuel resale prices | CSV | CNPJ, municipality, product, date | MT, legal persons |
| `comex` | Comex Stat municipal exports | CSV | `CO_MUN`, SH4, year/month | MT |
| `mt` | MT state convenios | CSV | convenio number, CNPJ | MT |

Common `.ocrec` identity columns: `record_id`, `geography_level`,
`geography_code`, `geography_name`, `cnpj`, `cnpj_root`, `period` and
`data_json`. The manifest records source/licence/URL, raw hash, schema, row
accounting and limitations.

## Commands

```bash
# Local raw file (preferred for reproducible evidence)
opencnpj-intel official ingest --adapter cnes \
  --dataset cnes-estabelecimentos \
  --input /data/opencnpj-data/intelligence/official/ingest/raw/cnes_estabelecimentos_csv.zip \
  --out /data/opencnpj-data/intelligence/official/ingest/national/cnes-estabelecimentos.ocrec \
  --scope all --period 2026-11

# Optional full download by URL
opencnpj-intel official ingest --adapter antt \
  --dataset antt-rntrc-veiculos \
  --out /data/opencnpj-data/intelligence/official/ingest/mt/antt-rntrc-veiculos.ocrec \
  --raw-out /data/opencnpj-data/intelligence/official/ingest/raw/antt.raw.json \
  --scope MT

opencnpj-intel official verify-records --dir /data/opencnpj-data/intelligence/official/ingest/national
opencnpj-intel official audit-records \
  --dir /data/opencnpj-data/intelligence/official/ingest/national \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced.ocnpj \
  --out /data/opencnpj-data/intelligence/official/ingest/audit/records-cnpj-audit-national.json
```

## Streaming sink

`RecordWriter` inserts each parsed record directly into the temporary SQLite
store inside one transaction. The adapters no longer materialize a record vector,
so full national runs stay bounded by the source buffer plus the current row.
Measured full national runs:

| Dataset | Records | Max RSS before | Max RSS with sink |
|---|---:|---:|---:|
| INEP 2024 | 215,545 | ~10.5 GB | 238 MB |
| PGFN Previdenciário | 3,287,586 | ~10.7 GB | 198 MB |
| PGFN FGTS | 549,458 | ~1.9 GB | 65 MB |

## MT runs

| Dataset | Records | Source rows | Out of scope | Rejected | Store bytes | Store SHA-256 |
|---|---:|---:|---:|---:|---:|---|
| `antt-rntrc-veiculos` | 4,533 | 98,119 | 93,586 | 0 | 1,916,928 | `1364a79e4fc0ab7958ca788f96c341e1a65dc18209fab38f20d7522bc64520c4` |
| `cnes-estabelecimentos` | 14,101 | 642,268 | 628,167 | 0 | 20,938,752 | `eb673a1917c1a6d076978c28a622e9a03c331ad5a412ea3bd1f5cbe358735386` |
| `inep-censo-escolar-2024` | 3,191 | 215,545 | 212,354 | 0 | 39,591,936 | `9bbc05425c87d85afd5a386ffe78a5388976c154522fa9606c86526bc5b814e0` |
| `pgfn-fgts-2026Q3` | 7,598 | 552,079 | 544,481 | 0 | 6,250,496 | `dca95fe3a47233f784add9996135fb9d2b2c20c587f0b62bcce5cfe8ba0f7962` |
| `pgfn-previdenciario-2026Q3` | 81,870 | 3,397,082 | 3,315,212 | 0 | 60,026,880 | `54fc88b5836a788e969a8eb3f5dc6cff170b9b89554f972f4cb3b57b9fa23e57` |
| `aneel-gd` | 15,651 | 4,735,908 | 4,720,257 | 0 | 22,953,984 | `2f5f63995f9265838d6facabf0fe183f8c9852bf4f8f8c9bc5198f8814b470a9` |
| `anp-precos-revenda` | 843 | 47,555 | 46,712 | 0 | 618,496 | `a7b392fe8aae94f15b0e9d71d3f7f77dfdb5ff126067230425ca183dfa938502` |
| `comex-exp-2025-mun` | 13,271 | 1,207,088 | 1,193,817 | 0 | 4,296,704 | `25fa3903acb29e7327325e3f4951f0304caf73ccb90fedb2ae0985e533d43620` |
| `mt-convenios-2024` | 79 | 79 | 0 | 0 | 114,688 | `f075bb13b29ea8e062064d4616976da495c29705c0de9fcde3e8a21ef908370b` |

## Full national runs

| Dataset | Records | Source rows | Out of scope | Rejected | Store bytes | Store SHA-256 |
|---|---:|---:|---:|---:|---:|---|
| `antt-rntrc-veiculos` | 98,119 | 98,119 | 0 | 0 | 41,459,712 | `012bc71670b3db2216e3856920b87ffc631620735962b0418cd91585fd93f69f` |
| `cnes-estabelecimentos` | 641,928 | 642,268 | 0 | 340 | 954,036,224 | `055ad700d361e8232f08fcf00d223cf67b8208075b49eca2e87c4385fa7e8818` |
| `inep-censo-escolar-2024` | 215,545 | 215,545 | 0 | 0 | 2,673,111,040 | `40ae4bb680f96c1d5451e9791bc40612973e183913ad7f3f6711793a27c870df` |
| `pgfn-fgts-2026Q3` | 549,458 | 552,079 | 2,621 | 0 | 451,731,456 | `de8bd16f72256c33f6c07c8e16c09370a15e6cb8d9d6b8af5cf7decbd7730eed` |
| `pgfn-previdenciario-2026Q3` | 3,287,586 | 3,397,082 | 109,496 | 0 | 2,416,336,896 | `fa18994994d98920f9d2b9f0499660b56cba8ac3222a472292149321257213c3` |
| `bcb-sgs-1` | 2,471 | 2,471 | 0 | 0 | 442,368 | `a1a01ad73087fbd6d698d3195814b38cc1a653b2db26ec19b2c194b56c3364e2` |
| `bcb-sgs-433` | 561 | 561 | 0 | 0 | 126,976 | `240bb6584a3c9327e6a92a4642eaab046b909cc16c2067c6604d45de2bfc2aa8` |

Raw evidence is outside Git under
`/data/opencnpj-data/intelligence/official/ingest/raw/`. The full national stores
are 6.1 GB and remain outside Git.

## Record-level CNPJ findings

`audit/records-cnpj-audit-national.json` sampled 5,000 records per store:

- CNES: 345,326 unique CNPJs; sampled CNPJs all present in the 2026-10 snapshot.
- INEP: 35,385 unique CNPJs; 2 sampled CNPJs absent from the snapshot.
- PGFN FGTS: 309,267 unique CNPJs; 1 invalid check-digit CNPJ found.
- PGFN Previdenciário: 824,940 unique CNPJs; 20 invalid check-digit CNPJs found.
- ANTT: no CNPJ identity in this aggregate resource.
- ANEEL: 1 invalid check-digit CNPJ and 1 CNPJ absent from the snapshot in the MT sample.
- ANP: all 126 unique MT CNPJs sampled were present; no invalid CNPJ found.
- MT convenios: all 68 unique CNPJs sampled were present.
- Comex Stat: no CNPJ identity; municipal export totals only.
- BCB: national series, no CNPJ.
- CNES national: 340 rows rejected because they use DF administrative-region
  codes (`530040`, `530050`, …) that are not in the IBGE municipality catalogue.
  The sample is recorded in the manifest limitations.
- INEP `99999999999999` sentinels are normalized to missing and are not joined.

Invalid or absent source CNPJs are kept flagged in the audit, not silently
repaired. They require source-side or crosswalk normalization before any join.

## Limitations

- The record writer is streaming. Large raw CSV entries are still buffered per
  file (ANEEL: ~1.5 GB) before row iteration, but records are not accumulated.
- `--scope MT` still reads every source row; out-of-scope rows are counted.
- PGFN natural-person records are excluded from this business slice.
- ANTT records are aggregate, not identified company fleets.
- CNES/INEP IDs are not CNPJs.
