# Commands and raw evidence

All commands run from `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`.
`CARGO_TARGET_DIR=/data/opencnpj-target` reuses the existing local Cargo cache.
Raw logs retain Cargo's own timings; this report makes no latency/RSS claim.

## Build and delivery gate

```sh
CARGO_TARGET_DIR=/data/opencnpj-target cargo build --release -p opencnpj-lab
cargo fmt --all --check
CARGO_TARGET_DIR=/data/opencnpj-target cargo clippy --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/data/opencnpj-target cargo test --workspace
```

Raw output: `build.log`, `fmt.log` (empty on success), `clippy.log`, `test.log`.
The report records the successful command exit statuses.

## National commercial round

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-01/questions.jsonl \
  --out docs/intelligence/results/commercial-round-01/national \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Raw output: `national.log`, `national/results.jsonl`,
`national/predictions.jsonl`, `national/summary.json`.
The snapshot's SHA-256 is recomputed and checked against the aggregate manifest;
the municipality catalogue hash is also verified. Expected decisions do not
substitute for predicted decisions. Gold IR is never fed to the interpreter.

## Previously frozen suites (text interpretation)

Run this command for each pair below, without editing its input:

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/adv.jsonl \
  --out docs/intelligence/results/commercial-round-01/frozen-adv \
  --interpret-only \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Input/output pairs: `adv.jsonl` → `frozen-adv`; `h-informal.jsonl` →
`frozen-informal`; `dialogues.jsonl` → `frozen-dialogues`; `ood.jsonl` →
`frozen-ood`. The matching `.log` beside each output directory is the raw CLI
stdout/stderr; every directory also retains all predictions and per-item
canonical comparisons. Dialogue lines expand into individual turns.

No H-blind run was made. No human input/approval was used to label this round.

## Local CLI conversation smoke check

```sh
/data/opencnpj-target/release/opencnpj-intel ask \
  --text 'Quantas padarias ativas existem na cidade de Sorriso/MT?' \
  --state /tmp/opencnpj-commercial-smoke-session.json \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
/data/opencnpj-target/release/opencnpj-intel ask \
  --text 'E em Sinop/MT?' \
  --state /tmp/opencnpj-commercial-smoke-session.json \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Raw JSON: `ask-smoke-01.json`, `ask-smoke-02.json`. Both executions succeeded.
The second request replaced the municipality while retaining the bakery activity.
