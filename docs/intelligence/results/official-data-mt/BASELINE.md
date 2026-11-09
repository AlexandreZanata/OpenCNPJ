# Official-data MT baseline (stage 0)

Recorded: 2026-11-08.

## Workspace

- Isolated worktree: `/tmp/opencnpj-official-data-mt`
- Branch: `codex/official-data-mt`, created from `origin/codex/official-data-mt-plan`
- Baseline commit: `27c2b92 docs(docs): define phased official data integration for MT`
- Remote: `origin git@github.com:AlexandreZanata/OpenCNPJ.git`
- The active `codex/commercial-round-09` worktree was not switched, reset,
  stashed or cleaned. Its uncommitted NLU/evaluator files are excluded from this
  branch. No claim is made that unfinished round 09 is validated.

## Gate

| Command | Exit |
|---|---:|
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0 |

Logs: `logs/stage-0-clippy.log`, `logs/stage-0-test.log`.

## Source and catalogue identities

| Asset | SHA-256 |
|---|---|
| `crates/opencnpj-insights/catalog/municipalities.tsv` | `d0bc5bbe308d35ae3b7913e617f0928ca5ac187464c49d517e11b4048d74c2b5` |
| `crates/opencnpj-insights/catalog/cnae.tsv` | `da1dce09b3a6a08ba922152f6760dabbd6c422907ff2ef3c8ce59795902b70f4` |
| `crates/opencnpj-insights/catalog/sectors.toml` | `cb8960afc7b1b59c5be38ce514b5634a1a2cfe4b7225adfb61efcf80f5b1eea3` |
| `/data/opencnpj-data/intelligence/agg/2026-10.ocagg` | `bd016a920eb141b1e260b826dc5b80a7b1397386ecce4074f5265d53b734845c` |

The RFB snapshot and aggregate in `/data/opencnpj-data` are local reference
assets and remain outside Git.

## Inventory reused

- Municipality, population-estimate, neighbour and IBGE-code catalogue in
  `opencnpj-insights::catalog::municipalities`.
- CNAE hierarchy, curated sectors and RFB reference extraction in
  `opencnpj-lab::catalog`.
- Aggregate-backed establishment queries in `opencnpj-insights::execute` and
  `opencnpj-lab::questions`.
- Existing `.ocnpj` and `.ocagg` formats are unchanged by this work.

## Scope of the new work

The official-data vertical is an independent, versioned sidecar. It does not
reinterpret municipal statistics as CNPJ-level facts, does not replace the
CNPJ/CNAE/history spine, and does not alter H-blind or frozen datasets.
