# Round 08 commands and evidence

Run from `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`.
No latency, throughput, memory or target-VPS claim is made.

## Completion of the original national run

The following command was already running when this review started. It was
preserved through completion, using its original loaded executable. Its hash
and the pre-review source hashes are in `baseline-provenance.json`.

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-08/questions.jsonl \
  --out docs/intelligence/results/commercial-round-08/national \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Raw evidence: `national.log`, `national/results.jsonl`,
`national/predictions.jsonl`, `national/summary.json`. This directory is the
original baseline, not a claim that it used the subsequently corrected parser.

## Final build and delivery checks

```sh
cargo fmt --all --check
CARGO_TARGET_DIR=/data/opencnpj-target cargo clippy --offline --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/data/opencnpj-target cargo test --offline --workspace
CARGO_TARGET_DIR=/data/opencnpj-target cargo build --offline --release -p opencnpj-lab
```

Exact arguments and exit codes are in `checks.json`. Raw output is in
`fmt.log`, `clippy.log`, `test.log` and `build.log`.

## Full text re-evaluation with the corrected interpreter

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-08/questions.jsonl \
  --out docs/intelligence/results/commercial-round-08/post-review-interpretation \
  --interpret-only \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Raw evidence: `post-review-interpretation.log` and all three output files in
`post-review-interpretation/`. The original labels were not rewritten.

`evaluation-runs.json` records the exact arguments, exit code and summary path
for all 13 evaluations: this full-set interpretation, four older frozen suites,
seven prior commercial-round replays, and the new review regressions. Frozen
suite inputs were `adv.jsonl`, `h-informal.jsonl`, `dialogues.jsonl` and
`ood.jsonl`; H-blind was not opened or run. Prior-round replay outputs remain
at the `/tmp/opencnpj-r08-final-replay-*` paths recorded in that JSON, with
complete stdout saved as `replay-round-*.log` in this directory.

## Focused review cases executed against the actual data

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-08/review-regressions.jsonl \
  --out docs/intelligence/results/commercial-round-08/review-regressions \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

The 20 review labels were authored from the source catalogues and calendar
arithmetic before obtaining predictions. Their separate manifest pins their
hash; they do not replace any original question. All per-item predicted and
reference outputs are retained.

## Audit and completion reconciliation

`dataset-audit.json` records source hash verification, composition, overlap,
reused session names and label-defect IDs. The 1,600 flagged original items
are closure questions containing the positive `com início de atividade`
restriction, while their labels omit `scope.opened` and use that restriction's
period as the closure period. Relative periods, accents and case are included.
These labels stay frozen.

`label-adjudications.jsonl` contains the 1,600 explicit replacement decisions,
each bound to the original dataset and gold-label hashes, with semantic reasons
and synthetic authorship. It is a separate overlay, not an edit to the original
questions. Any agreement measured with that overlay is disclosed separately.

`finalize.py` reconciles every original question, completed baseline record and
final prediction in order. It requires exactly 50,000 distinct matching IDs,
checks the data/dataset identities, and verifies successful existing execution
for every unchanged accepted typed IR. This is explicit reuse of baseline
backend evidence, not a second national execution with the corrected parser.
It also verifies all 20 newly executed review cases, all build/test exit codes,
and the final mismatch partition before writing the completion index and hashes.

```sh
python3 docs/intelligence/results/commercial-round-08/finalize.py
```

The finalizer uses only the Python standard library for evidence reconciliation;
the interpreter and evaluation CLI continue to run in Rust without Python.
