# 12 — Benchmark protocol

Every number used to choose the architecture is produced by this protocol, with its
command and raw output kept (AGENTS.md). All variants (A, B, C, D, E1) run on the **same**
frozen sets, the same machine, the same day where possible.

## 1. Harness

Binary `opencnpj-intel` (crate `crates/opencnpj-lab`):

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target            # / is full on the build host
cargo build --release -p opencnpj-lab
B=$CARGO_TARGET_DIR/release/opencnpj-intel
D=/data/opencnpj-data/intelligence

$B synth  --spec crates/opencnpj-lab/synth/pt-BR --seed 42 --size 100000 --out $D/datasets
$B eval   --variant A2 --lang-pack $D/packs/lang-pt-BR.bin --sets $D/eval --out $D/results/A2
$B bench-nlu --variant A2 --set $D/eval/h-dev.jsonl --iterations 20000 --seed 42 --out $D/results/A2-latency
$B bench-agg --agg $D/agg/2026-10.ocagg --queries 10000 --seed 42 --out $D/results/agg-latency
$B diff-agg  --agg $D/agg/2026-10.ocagg --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj --sample 10000 --seed 42
```

Outputs: `results.json` (all metrics, per-item predictions), `results.md` (tables),
`env.json` (git commit, rustc, CPU model, governor, core used, load average, RSS).
Raw outputs stay under `/data/opencnpj-data/intelligence/results/` (outside Git); the
summary with commands is committed as `docs/intelligence/results/<date>-<topic>.md`.

## 2. Evaluation sets

S-test (template-family and slot-value hold-out), H-dev, H-blind, ADV, OOD, H-informal,
dialogue scripts — defined in [10-SYNTHETIC-DATA.md](10-SYNTHETIC-DATA.md). H-blind runs
only at milestones (decision, release), never during tuning.

## 3. Quality metrics (exact definitions)

| Metric | Definition |
|---|---|
| Intent accuracy, macro-F1 | Over in-domain items; per-intent precision/recall/F1 reported |
| Entity P/R/F1 | A predicted entity is correct when span **and** resolved catalogue id match the gold |
| Full-plan exact match | Canonical IR byte-equality with the gold IR (04-QUERY-IR.md §3) |
| Coverage | executed items / in-domain items whose gold decision is `execute` |
| Accepted error rate (risk) | executed items whose IR ≠ gold / executed items. **The primary safety metric** |
| Clarification recall | items with gold `clarify` that were clarified (not executed, not rejected) |
| Unnecessary clarification | items with gold `execute` that were clarified |
| OOD rejection | OOD items rejected / OOD items; false rejection = in-domain items rejected |
| Negation errors | ADV negation slice: items where a negated constraint was dropped or inverted |
| Context accuracy | dialogue: per-turn gold state equality; **filter leaks** (inherited filter on a `NewQuery`) counted separately, target 0 |
| Risk–coverage | curve over the policy thresholds; AURC reported |
| Calibration | ECE and reliability diagram (classifier variants) |

Every metric is reported overall and per slice (formal, colloquial, informal/typos,
homonyms, negation, compound, follow-ups) with the count of items per cell.

## 4. Resource metrics

| Metric | How |
|---|---|
| Latency per stage | `Instant` around normalize, spot, constructions, scorer, build+validate, (execute, render when end to end); 1 untimed warm-up pass; 20,000 utterances sampled with replacement from H-dev + S-test; p50/p95/p99/max |
| Interpretation vs query vs total | Reported separately (brief §10): interpretation = text → valid IR; query = IR → result; total = request in → JSON out (in process), plus HTTP end to end with `opencnpj loadtest` |
| Throughput | single thread, and on the 1-vCPU emulation (§6) with 1, 4, 16 concurrent workers |
| Memory | RSS before/after loading the language pack, classifier and aggregates (`VmRSS`, `VmHWM`, `RssAnon`, `RssFile` from `/proc/self/status`); allocations per utterance (counting allocator in the bench binary only) |
| Artifact size | bytes and SHA-256 of language pack, classifier, aggregates |
| Startup | process start → ready, cold (page cache evicted with `posix_fadvise`, as `bench --cold`) and warm |
| Training time | wall and CPU time per variant and dataset size |
| Update complexity | for the "add two intents" exercise: files and lines changed, examples written, regressions on frozen sets |
| Cost | measured CPU-seconds per 1,000 questions × the VPS price (provider and date recorded); no estimate without the measured CPU time |

## 5. Learning curves and diversity

Grid: size ∈ {1k, 3k, 10k, 30k, 100k, 300k} × diversity ∈ {L1, L2, L3} × seed ∈ {1, 2, 3}
for each learned variant (B-LR at least; others if time permits). Metrics on S-test and
H-dev: full-plan exact match, intent macro-F1, accepted error at the fixed policy.
**Plateau** = less than 0.5 pp gain per doubling of size for two consecutive doublings.
The report states the plateau point per diversity level, i.e. whether more templates beat
more examples.

## 6. Emulating the 1 vCPU / 1 GB node

On the build host (CPUs 12–15 are E-cores at up to 3.6 GHz, a conservative stand-in for
a VPS vCPU):

```bash
systemd-run --user --scope -p MemoryMax=1G -p MemorySwapMax=0 -p CPUQuota=100% \
  taskset -c 12 $B bench-nlu --variant A2 --set $D/eval/h-dev.jsonl --iterations 20000 --out $D/results/A2-vps
```

Check inside the run: `Cpus_allowed_list: 12` in `/proc/self/status`; memory limit via
`systemctl --user show <scope> -p MemoryMax`. If user-level memory delegation is not
available, run the same command with `sudo systemd-run --scope …` and record which was
used. Run each configuration 3 times; report the median of the p95s and the spread, plus
the host load average (the build host is a shared desktop, see BENCHMARKS.md). Before
launch, repeat the key runs on a real 1 vCPU / 1 GB VPS (WP-6.6) — emulation is not a
substitute for the target.

## 7. Thresholds used by the decision

| Requirement | Threshold | Set |
|---|---|---|
| NFR-07 accepted precision | ≥ 99 % | H-blind |
| NFR-08 coverage | ≥ 85 % | H-blind |
| NFR-09 OOD rejection | ≥ 95 % | OOD |
| Filter leaks | 0 | dialogue scripts |
| NFR-01 interpretation p95 | < 10 ms (goal < 1 ms) | 1-vCPU emulation |
| NFR-02 aggregate query p95 | < 10 ms | 1-vCPU emulation |
| NFR-04 interpreter RSS | < 20 MB | 1-vCPU emulation |
| NFR-05 classifier artifact | < 5 MB | — |
| NFR-12 aggregates vs engine | 0 mismatches | `diff-agg` |

## 8. Result report template

```markdown
# <date> — <experiment>  [MEASURED]
Commit: <sha>  Host: <cpu, core used, governor>  Load avg: <…>  rustc: <…>
Sets: <dataset ids and SHA-256>
## Commands
<exact commands>
## Raw output
<verbatim tables or path to results.json under /data/opencnpj-data/intelligence/results/>
## Summary
<table per metric and slice>
## Notes
<anything unusual; what is an estimate; what was not measured>
```
