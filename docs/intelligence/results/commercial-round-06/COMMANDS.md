# Commands and raw evidence

All commands run from `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`.
`CARGO_TARGET_DIR=/data/opencnpj-target` reuses the existing local Cargo cache.
Raw logs retain the tools' own output; this report makes no latency/RSS claim.

## Question authoring (one-off, outside the repository)

```sh
mkdir -p /tmp/opencode/r06
# Deterministic panel builders (local, uncommitted):
R06_PANEL=0 python3 -I /tmp/opencode/r06/make_round06_panel.py
R06_PANEL=1 python3 -I /tmp/opencode/r06/make_round06_panel.py
python3 -I /tmp/opencode/r06/make_round06_merge.py
```

The two panels use different catalogue index shifts, city orders, OOD slices,
historical month pairs, rank sizes and CNPJ roots. The merge step repairs only
filler words for texts that collide with rounds 01-05 or across panels; slots,
filters and gold IRs are never changed. It writes
`crates/opencnpj-lab/eval/commercial-round-06/{questions.jsonl,questions.tsv,manifest.json,DATASET-CARD.md}`.
No third-party dataset, network access or new dependency is used.

## Build and delivery gate

```sh
CARGO_TARGET_DIR=/data/opencnpj-target cargo build --release -p opencnpj-lab
cargo fmt --all --check
CARGO_TARGET_DIR=/data/opencnpj-target cargo clippy --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/data/opencnpj-target cargo test --workspace
```

Raw output: `build.log`, `fmt.log` (empty on success), `clippy.log`, `test.log`.
The final test log records 294 passed, 0 failed, 5 ignored.

## National commercial round

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-06/questions.jsonl \
  --out docs/intelligence/results/commercial-round-06/national \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Raw output: `national.log`, `national/results.jsonl`,
`national/predictions.jsonl`, `national/summary.json`.
The runner receives text plus its own prior predictions; `gold.ir` and
`context.previous_ir` are never interpreter inputs. The same command was used
for the first scored pass and for the final pass after the general fixes.

## Previously frozen suites (text interpretation)

Run this command for each pair below, without editing its input:

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/adv.jsonl \
  --out docs/intelligence/results/commercial-round-06/frozen-adv \
  --interpret-only \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Input/output pairs: `adv.jsonl` -> `frozen-adv`; `h-informal.jsonl` ->
`frozen-informal`; `dialogues.jsonl` -> `frozen-dialogues`; `ood.jsonl` ->
`frozen-ood`. Each output directory retains predictions, per-item canonical
comparisons and the CLI summary. The matching `.log` beside it is raw stdout.

Earlier commercial rounds were replayed with the final binary and
`--interpret-only`; output directories stayed outside the repository:

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-05/questions.jsonl \
  --out /tmp/opencode/r06/replay-r05 \
  --interpret-only \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
# ... same for rounds 01-04 (/tmp/opencode/r06/replay-r01 ... replay-r04)
```

Scores: round 01 1000/1000, round 02 1000/1000, round 03 2994/3000,
round 04 4987/5000, round 05 9975/10000. The round-04 and round-05 replays gain
one and two items respectively from the entity-aware cue fix; no prior-round
score regressed.

## Local CLI conversation smoke check

```sh
/data/opencnpj-target/release/opencnpj-intel ask \
  --text 'Quantas padarias ativas existem na cidade de Sorriso/MT?' \
  --state /tmp/opencnpj-commercial-r06-smoke-session.json \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
/data/opencnpj-target/release/opencnpj-intel ask \
  --text 'E em Sinop/MT?' \
  --state /tmp/opencnpj-commercial-r06-smoke-session.json \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Raw JSON: `ask-smoke-01.json`, `ask-smoke-02.json`. Both executions succeeded
(scalars 152 and 321). The second request replaced the municipality while
retaining the bakery activity.

No H-blind path was opened or executed. No human input/approval was used to
label this round. No commit or push was made.
