# Round 10 Commands and Evidence

Run from `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`.
No latency, throughput, memory or target-VPS claim is made.

One heavy evaluator ran at a time. `H-blind` was neither opened nor executed.
`results.jsonl` and `predictions.jsonl` from the 50,000-item national run are kept
outside Git in `/data/opencnpj-data/intelligence/results/commercial-round-10/national/`
with hashes recorded in `evidence-sha256.json` and `evaluation-runs.json`.

## 1. Source registry, coverage plan and authoring verification

```sh
python3 crates/opencnpj-lab/eval/commercial-round-10/build_plan_artifacts.py
python3 crates/opencnpj-lab/eval/commercial-round-10/author_round10.py --pilot-only
python3 crates/opencnpj-lab/eval/commercial-round-10/review_round10.py \
  --set crates/opencnpj-lab/eval/commercial-round-10/batches/batch-001.jsonl \
  --out docs/intelligence/results/commercial-round-10/pilot-label-review.json
python3 crates/opencnpj-lab/eval/commercial-round-10/author_round10.py --all
python3 crates/opencnpj-lab/eval/commercial-round-10/review_round10.py \
  --set crates/opencnpj-lab/eval/commercial-round-10/questions.jsonl \
  --out docs/intelligence/results/commercial-round-10/dataset-audit.json --full-50k
```

Raw evidence: `PLAN.md`, `source-registry.json`, `regional-glossary.json`,
`prior-rounds-gap-inventory.json`, `coverage-matrix.json`, `pilot-label-review.json`,
`dataset-audit.json`, `crates/opencnpj-lab/eval/commercial-round-10/manifest.json`,
`batch-manifests.json` and `DATASET-CARD.md`.

## 2. Pilot evaluation (`batch-001.jsonl`, 1,000 items)

Before NLU fixes (initial pilot revision; hash is recorded in the summary):

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-10/batches/batch-001.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/pilot-before-fix
```

After NLU and authoring fixes:

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-10/batches/batch-001.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/pilot-after-fix
```

Raw evidence: `pilot-before-fix/summary.json`, `pilot-after-fix/summary.json`,
`pilot-after-fix.log`.

## 3. National evaluation (`questions.jsonl`, 50,000 items)

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-10/questions.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/national
```

Raw evidence: `national.log`, `national/summary.json`, plus per-item outputs in
`/data/opencnpj-data/intelligence/results/commercial-round-10/national/{results,predictions}.jsonl`
(kept outside Git).

## 4. Frozen suites (`adv.jsonl`, `h-informal.jsonl`, `dialogues.jsonl`, `ood.jsonl`)

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/adv.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/frozen-adv

/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/h-informal.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/frozen-informal

/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/dialogues.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/frozen-dialogues

/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/ood.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-10/frozen-ood
```

Raw evidence: `frozen-{adv,informal,dialogues,ood}.log` and
`frozen-{adv,informal,dialogues,ood}/summary.json`.

## 5. Prior commercial rounds replay (`commercial-round-01` .. `commercial-round-09`)

```sh
for r in 01 02 03 04 05 06 07 08 09; do
  /data/opencnpj-target/release/opencnpj-intel eval-questions \
    --set crates/opencnpj-lab/eval/commercial-round-$r/questions.jsonl \
    --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
    --history /data/opencnpj-data/intelligence/backfill/aggregates \
    --interpret-only \
    --out /data/opencnpj-data/intelligence/results/commercial-round-10/replay-round-$r
done
```

Raw evidence: `replay-round-{01..09}.log` and `prior-rounds-replay.json`.

## 6. Cargo delivery gate and final reconciliation

```sh
cargo fmt --all --check
CARGO_TARGET_DIR=/data/opencnpj-target cargo clippy --offline --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/data/opencnpj-target cargo test --offline --workspace
CARGO_TARGET_DIR=/data/opencnpj-target cargo build --offline --release -p opencnpj-lab
python3 crates/opencnpj-lab/eval/commercial-round-10/finalize_round10.py
```

Raw evidence: `fmt.log`, `clippy.log`, `test.log`, `build.log`, `checks.json`,
`evaluation-runs.json`, `metamorphic-checks.json`, `split-breakdown.json`,
`index.json`, `final-source-sha256.json`, `evidence-sha256.json`.
