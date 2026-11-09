# 2026-11-06 — WP-3.2 evaluation harness [MEASURED]

Base commit `19f49166a8c583884cf8d54c3635006d7b544695`. Compiler
`rustc 1.99.0 (b940084d7 2026-09-28)`.

## Delivery

`opencnpj-lab::eval` implements the §1 harness for the frozen sets:

* `eval` computes intent accuracy and macro-F1, entity P/R/F1, full-plan
  exact match, coverage, accepted error (risk), clarification recall,
  unnecessary clarification, OOD rejection, false rejection, negation
  errors, context accuracy and filter leaks, the risk–coverage curve with
  AURC, and ECE, overall and per slice.
* `bench-nlu` measures a predictor's latency (p50/p95/p99/max) and records
  the environment; `bench-agg` is the aggregate/engine benchmark from
  WP-2.3 under the protocol name.
* `eval` writes `results.json` (metrics, risk–coverage points,
  predictions, environment), `results.md` and `env.json`.
* `crates/opencnpj-lab/scripts/vps-emulate.sh` runs a command on one vCPU
  with `MemoryMax=1G`, falling back to `sudo systemd-run` when the user
  scope cannot set limits. The run reports the allowed cores.
* The trivial `count` baseline (no entities) is built from the production
  IR: active establishments, current competence, no place or activity.

The H-blind guard refuses a set whose name contains `h-blind` unless
`--milestone` is passed; H-dev and synthetic sets run without it.

## Unit tests

`cargo test -p opencnpj-lab --bin opencnpj-intel eval::tests`:

```text
metrics_match_hand_computed_example ... ok
baseline_produces_expected_numbers ... ok
milestone_gates_h_blind ... ok
negation_and_context_are_counted ... ok
```

The hand-computed example fixes the risk–coverage points
(`1/3, 0`), (`2/3, 0.5`), (`1, 1/3`), AURC `5/18`, ECE `0.4`; the
baseline example fixes coverage `1`, risk `1`, exact `0`, entity `0`,
AURC `1`, ECE `0.5`.

## H-dev baseline [MEASURED]

Set: `/data/opencnpj-data/intelligence/eval/h-dev.jsonl`, 317 items,
SHA-256 `c5a98077a6470e3b15a9c4c988be6be34e9965dbc8f2ce057cfc27bf45e45ebd`.

```bash
B=/data/opencnpj-target/release/opencnpj-intel
D=/data/opencnpj-data/intelligence
$B eval --set $D/eval/h-dev.jsonl --baseline count \
  --out $D/results/2026-11-06-baseline-h-dev
```

```text
items 317, in-domain 292, executed 292
exact 0.000, risk 1.000, coverage 1.000
intent accuracy 0.168, macro-F1 0.036
entity P/R/F1 0, negation 20/20, context 31 items (accuracy 0)
AURC 1.000, ECE 0.500
```

The baseline executes everything, so coverage is 100 % and every executed
plan is wrong: accepted error is 100 %. It predicts only `count`, which is
the expected 49/292 = 16.8 % intent accuracy and the count-only macro-F1
0.287 across eight intents. The H-dev gold has 292 executable items, 8
clarifications and 17 rejections; the baseline clarifies and rejects
nothing.

Per intent (gold, predicted, correct, F1): count 49/292/49/0.287; compare
36/0/0/0; list 36/0/0/0; rank 35/0/0/0; breakdown 34/0/0/0; lookup
34/0/0/0; trend 34/0/0/0; meta 34/0/0/0.

Slices: follow-up 56 items (risk 1, exact 0); negation 19 items (risk 1,
exact 0). The national H-dev has no register or noise metadata.

Raw output is in
`/data/opencnpj-data/intelligence/results/2026-11-06-baseline-h-dev/`
(`results.json`, `results.md`, `env.json`) and
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.2-eval.log`.

## 1-vCPU benchmarks [MEASURED]

The emulation used the project script with CPU 12:

```bash
VPS_CPU=12 crates/opencnpj-lab/scripts/vps-emulate.sh \
  /data/opencnpj-target/release/opencnpj-intel bench-nlu \
  --set $D/eval/h-dev.jsonl --iterations 20000 --seed 42 \
  --out $D/results/2026-11-06-baseline-h-dev-nlu
```

```text
variant baseline-count, 20000 iterations
p50 643 ns, p95 1242 ns, p99 1433 ns, max 18724 ns
Cpus_allowed_list = 1 core; governor performance; load 5.34
RSS/HWM 9348 kB
```

`bench-agg` (the WP-2.3 aggregate/engine executor on the golden set) under
the same script:

```text
case                        backend       aggregate_us  engine_us
padarias_sorriso            Aggregates          386       49329
empresas_abertas_2025_mt    Aggregates       502953       73485
near_sinop_km               Aggregates       124969       10958
rank_cnae_section           Aggregates       163046     1573278
total                                        3054344     7876193
```

The aggregate/engine measurements include the one-vCPU and 1 GiB scope.
The aggregate executor still scans cells; the WP-2.3 marginal fast path is
future work.

Raw output:
`/data/opencnpj-data/intelligence/results/2026-11-06-baseline-h-dev-nlu/bench-nlu.json`,
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.2-bench-nlu.log`,
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.2-bench-agg.log`.

## Delivery gate

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p opencnpj-lab --bin opencnpj-intel eval::tests
cargo test -p opencnpj-lab --bin opencnpj-intel synth::tests
git diff --check
```

All passed; the log is
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.2-gate.log`.

## Limits and follow-ups

* The A2/B/D interpreters are Phase 4; `eval` and `bench-nlu` currently run
  the trivial baseline. The harness is ready for their predictions.
* H-blind has not been collected (WP-0.3); the `--milestone` guard is
  tested with a synthetic blind path.
* The H-dev corpus is 50 human-reviewed and 267 AI-drafted items; the
  baseline evaluation above is descriptive, not a decision measurement.
* The ADV, OOD, H-informal and dialogue sets are WP-3.3; the harness
  supports them through the same JSONL interface.
* NFR-01/NFR-02 thresholds are not claimed: the baseline is not the
  interpreter, and the aggregate executor is still the full-scan version.
* The emulation ran through the user `systemd-run --scope`; the bench-nlu
  environment reported one allowed core and the 1 GiB memory cap was
  applied by the script.
