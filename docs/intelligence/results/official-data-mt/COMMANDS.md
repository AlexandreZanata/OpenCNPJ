# Commands and evidence index

All commands ran in the isolated worktree
`/tmp/opencnpj-official-data-mt` on 2026-11-08. `CARGO_TARGET_DIR` pointed at the
existing workspace target to reuse the Cargo cache.

## Delivery gate

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Exit codes: 0 for each. Logs: `logs/stage-0-*`, `logs/stage-1-*`,
`logs/stage-2-*`, `logs/stage-3-5-*`, `logs/stage-6-*`, `logs/stage-10-*`.

## Source lifecycle

```bash
opencnpj-intel sources list
opencnpj-intel sources status --dir /data/opencnpj-data/intelligence/official
opencnpj-intel sources verify --dir /data/opencnpj-data/intelligence/official
```

## IBGE imports

```bash
opencnpj-intel official import-ibge --dataset ibge-6579 --periods all \
  --out /data/opencnpj-data/intelligence/official/ibge-6579.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-6579.raw.json
opencnpj-intel official import-ibge --dataset ibge-5938 --periods all \
  --out /data/opencnpj-data/intelligence/official/ibge-5938.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-5938.raw.json
opencnpj-intel official import-ibge --dataset ibge-9509 --periods all \
  --out /data/opencnpj-data/intelligence/official/ibge-9509.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-9509.raw.json
opencnpj-intel official import-ibge --dataset ibge-9922 --periods all \
  --out /data/opencnpj-data/intelligence/official/ibge-9922.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-9922.raw.json
opencnpj-intel official import-ibge --dataset ibge-1612 --periods 2020-2025 \
  --out /data/opencnpj-data/intelligence/official/ibge-1612.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-1612.raw.json
opencnpj-intel official import-ibge --dataset ibge-1613 --periods 2020-2025 \
  --out /data/opencnpj-data/intelligence/official/ibge-1613.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-1613.raw.json
opencnpj-intel official import-ibge --dataset ibge-3939 --periods 2020-2025 \
  --out /data/opencnpj-data/intelligence/official/ibge-3939.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-3939.raw.json
opencnpj-intel official import-ibge --dataset ibge-6588 --periods all \
  --out /data/opencnpj-data/intelligence/official/ibge-6588.ocind \
  --raw-out /data/opencnpj-data/intelligence/official/raw/ibge-6588.raw.json
```

## RAIS and Senatran

```bash
opencnpj-intel official import-rais \
  --xlsx /data/opencnpj-data/intelligence/official/raw/mte-rais-2024.raw.xlsx \
  --out /data/opencnpj-data/intelligence/official/mte-rais-2024.ocind
opencnpj-intel official import-senatran \
  --xlsx /data/opencnpj-data/intelligence/official/raw/senatran-frota-2026-09.raw.xlsx \
  --out /data/opencnpj-data/intelligence/official/senatran-frota-2026-09.ocind
```

## Real queries

See `logs/stage-2-real-queries.log`, `logs/stage-3-rais-query.log` and
`logs/stage-5-senatran-queries.log`. Examples:

```bash
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation lookup --variable populacao_estimada --geography 5107925 --period 2024
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation rank --variable pam_producao --classification 81 --category 2713 \
  --period 2024 --uf MT --limit 10
opencnpj-intel official query --dir /data/opencnpj-data/intelligence/official \
  --operation trend --variable lspa_producao --classification 48 --category 39443 \
  --geography 51 --period 202601 --period 202605 --period 202609
```

## Portuguese questions

```bash
opencnpj-intel official ask --dir /data/opencnpj-data/intelligence/official \
  --text "Qual a população estimada de Sorriso em 2024?"
opencnpj-intel official ask --dir /data/opencnpj-data/intelligence/official \
  --text "Evolução da produção de soja em Sorriso entre 2020 e 2024"
opencnpj-intel official ask --dir /data/opencnpj-data/intelligence/official \
  --text "Quantos funcionários tem o CNPJ de Sorriso?"
```

## Additional source and CNPJ crosswalk audit

```bash
opencnpj-intel official audit-sources \
  --out-dir /data/opencnpj-data/intelligence/official/audit \
  --json-out /data/opencnpj-data/intelligence/official/audit/source-audit.json
opencnpj-intel official audit-cnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --catalog crates/opencnpj-insights/catalog/municipalities.tsv \
  --stores /data/opencnpj-data/intelligence/official \
  --out /data/opencnpj-data/intelligence/official/audit/cnpj-crosswalk-audit.json
```

Result: 24 probes, 0 network errors, 0 unmatched CNPJ municipality IDs. See
`SOURCE-AUDIT.md` and `INCONSISTENCIES.md`.

## Acceptance corpus

```bash
python3 scripts/official-data/run_acceptance.py \
  --bin /home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ/target/debug/opencnpj-intel \
  --dir /data/opencnpj-data/intelligence/official
```

Result: 261/261 passed. Outcome: `acceptance/results.json`.

## HTTP API

```bash
opencnpj serve --snapshot /data/opencnpj-data/snapshots/2026-10-balanced.ocnpj \
  --official-dir /data/opencnpj-data/intelligence/official \
  --auth off --bind 127.0.0.1:18080 --admin-bind ''
curl 'http://127.0.0.1:18080/api/v1/official/sources'
curl 'http://127.0.0.1:18080/api/v1/official/query?operation=lookup&variable=populacao_estimada&geography=5107925&period=2024'
```

Observed: 10 imported stores, 10 datasets; query returned `ok`, `Pessoas`,
`120985.0`, source `ibge-6579`. Log: `logs/stage-10-http-api.log`.
