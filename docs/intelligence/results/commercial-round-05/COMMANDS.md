# Commands and raw evidence

All commands run from `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`.
`CARGO_TARGET_DIR=/data/opencnpj-target` reuses the existing local Cargo cache.
Raw logs retain Cargo's own timings; this report makes no latency/RSS claim.

## Question authoring (one-off, uncommitted)

```sh
python3 -I /tmp/opencode/r05/make_round05.py
```

Local deterministic script (kept outside the repository; no new dependency).
Adapted from the round-04 authoring script with new shifts (city 177, sector
101, shuffle seed 20261108), a disjoint OOD city slice, doubled counts and new
market-size leads. Reads `crates/opencnpj-insights/catalog/sectors.toml`
(213 sectors) and `municipalities.tsv` (250 main-pool cities across all 27 UFs
plus a disjoint OOD pool) and writes
`crates/opencnpj-lab/eval/commercial-round-05/questions.jsonl` (exactly 10,000
records, distinct normalized texts, 20 categories of 500, decisions
8950/500/550, no text overlap with rounds 01–04), `questions.tsv`,
`manifest.json` and `DATASET-CARD.md`.

Scale-up repairs are filler-only and never touch slots or gold IRs: sparse
templates whose slot cycles repeat within 500 items (national branches, UF
ranks, bare dialogue turns) are disambiguated with noise-slice fillers
(`por favor`, `bom dia`, safe abbreviations, light typo, `??`); prior-round
overlaps are repaired the same way. Tags `dedup_filler` and `overlap_repair`
record these text-only repairs.

## Build and delivery gate

```sh
CARGO_TARGET_DIR=/data/opencnpj-target cargo build --release -p opencnpj-lab
cargo fmt --all --check
CARGO_TARGET_DIR=/data/opencnpj-target cargo clippy --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=/data/opencnpj-target cargo test --workspace
```

Raw output: `build.log`, `fmt.log` (empty on success), `clippy.log`, `test.log`.
The report records the successful command exit statuses (293 passed, 0 failed, 5 ignored).

## National commercial round

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-05/questions.jsonl \
  --out docs/intelligence/results/commercial-round-05/national \
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
  --out docs/intelligence/results/commercial-round-05/frozen-adv \
  --interpret-only \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Input/output pairs: `adv.jsonl` → `frozen-adv`; `h-informal.jsonl` →
`frozen-informal`; `dialogues.jsonl` → `frozen-dialogues`; `ood.jsonl` →
`frozen-ood`. The matching `.log` beside each output directory is the raw CLI
stdout/stderr; every directory also retains all predictions and per-item
canonical comparisons. Dialogue lines expand into individual turns.

Earlier-round replays (reference only, output kept outside the repository):

```sh
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-01/questions.jsonl \
  --out /tmp/opencode/r05/r01replay \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
# ... same for commercial-round-02 (/tmp/opencode/r05/r02replay),
# commercial-round-03 (/tmp/opencode/r05/r03replay) and
# commercial-round-04 (/tmp/opencode/r05/r04replay)
```

Rounds 01 and 02 scored 1,000/1,000 interpretations, round 03 scored
2,994/3,000 and round 04 scored 4,986/5,000, identical to their frozen reports.

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
