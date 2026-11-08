# Round 09 Commands and Evidence

Run from `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`.
No latency, throughput, memory, or target-VPS claim is made.

## 1. Source Registry, Coverage Plan, and Authoring Verification

```sh
python3 crates/opencnpj-lab/eval/commercial-round-09/build_plan_artifacts.py
python3 crates/opencnpj-lab/eval/commercial-round-09/author_round09.py
python3 crates/opencnpj-lab/eval/commercial-round-09/review_round09.py
```

Raw evidence:
- `docs/intelligence/results/commercial-round-09/PLAN.md`
- `docs/intelligence/results/commercial-round-09/source-registry.json`
- `docs/intelligence/results/commercial-round-09/regional-glossary.json`
- `docs/intelligence/results/commercial-round-09/prior-rounds-gap-inventory.json`
- `docs/intelligence/results/commercial-round-09/coverage-matrix.json`
- `docs/intelligence/results/commercial-round-09/pilot-label-review.json`
- `docs/intelligence/results/commercial-round-09/batch-manifests.json`
- `docs/intelligence/results/commercial-round-09/dataset-audit.json`
- `crates/opencnpj-lab/eval/commercial-round-09/manifest.json`
- `crates/opencnpj-lab/eval/commercial-round-09/DATASET-CARD.md`

## 2. Pilot Evaluation (`batch-001.jsonl`, 1,000 Items) Before and After NLU Fixes

Before NLU fixes:

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-09/batches/batch-001.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/pilot-before-fix
```

After NLU fixes:

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-09/batches/batch-001.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/pilot-after-fix
```

Raw evidence: `pilot-before-fix.log`, `pilot-before-fix/summary.json`, `pilot-after-fix.log`, `pilot-after-fix/summary.json`.

## 3. National Evaluation (`questions.jsonl`, 50,000 Items)

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-09/questions.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/national
```

Raw evidence: `national.log`, `national/summary.json`, and full per-item outputs in `/data/opencnpj-data/intelligence/results/commercial-round-09/national/{results,predictions}.jsonl` (kept outside Git per national-artifact policy).

## 4. Frozen Suites (`adv.jsonl`, `h-informal.jsonl`, `dialogues.jsonl`, `ood.jsonl`)

`H-blind` was neither opened nor executed.

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/adv.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/frozen-adv

/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/h-informal.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/frozen-informal

/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/dialogues.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/frozen-dialogues

/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/ood.jsonl \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --out /data/opencnpj-data/intelligence/results/commercial-round-09/frozen-ood
```

Raw evidence: `frozen-{adv,informal,dialogues,ood}.log` and `frozen-{adv,informal,dialogues,ood}/summary.json`.

## 5. Prior Commercial Rounds Replay (`commercial-round-01` .. `commercial-round-08`)

```sh
for r in 01 02 03 04 05 06 07 08; do
  /data/opencnpj-target/release/opencnpj-intel eval-questions \
    --set crates/opencnpj-lab/eval/commercial-round-$r/questions.jsonl \
    --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
    --history /data/opencnpj-data/intelligence/backfill/aggregates \
    --interpret-only \
    --out /data/opencnpj-data/intelligence/results/commercial-round-09/replay-round-$r
done
```

Raw evidence: `replay-round-{01..08}.log` and `prior-rounds-replay.json`.

## 6. Cargo Delivery Gate and Final Reconciliation

```sh
cargo fmt --all --check
CARGO_TARGET_DIR=/data/opencnpj-target cargo clippy --offline --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/data/opencnpj-target cargo test --offline --workspace
CARGO_TARGET_DIR=/data/opencnpj-target cargo build --offline --release -p opencnpj-lab
python3 crates/opencnpj-lab/eval/commercial-round-09/finalize_round09.py
```

Raw evidence: `fmt.log`, `clippy.log`, `test.log`, `build.log`, `checks.json`, `evaluation-runs.json`, `metamorphic-checks.json`, `split-breakdown.json`, `index.json`, `final-source-sha256.json`, `evidence-sha256.json`.
