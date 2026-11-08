# Repeatable commercial-question evaluation

Start at the repository root. Keep question authorship synthetic until the owner
starts human product validation. Never mark synthetic questions as human data.
Do not commit or push unless asked. Keep H-blind untouched.

## Reproduce round 01

```sh
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo build --release -p opencnpj-lab
/data/opencnpj-target/release/opencnpj-intel eval-questions \
  --set crates/opencnpj-lab/eval/commercial-round-01/questions.jsonl \
  --out /tmp/commercial-round-01-replay \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

An optional `--interpret-only` run scores language understanding without engine
execution. Still provide aggregate/history files if historical capabilities are
being evaluated. Without them, the interpreter correctly considers only the
current competence available. Catalogue paths and `--anchor` can be overridden.

The output directory contains `results.jsonl`, `predictions.jsonl`, and
`summary.json`. One input record produces one output record. For dialogue files
with `turns`, one output record is produced per turn. Inspect actual per-item
errors, wrong accepted requests, zero-result searches and reference mismatches;
do not report only the interpretation headline.

The CLI reads text and its own previous predictions. Do not seed conversation
state or predictions with `gold.ir`, `context.previous_ir`, or expected result
values. Do not score an always-count or always-reject baseline as the product.

## Ask a question or follow up

```sh
/data/opencnpj-target/release/opencnpj-intel ask \
  --text 'Quantas padarias ativas existem na cidade de Sorriso/MT?' \
  --state /tmp/opencnpj-sales-session.json \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates
```

Repeat with the same state path and `--text 'E em Sinop/MT?'`. The state file is
local CLI state; it is not the planned signed HTTP conversation token. The
interpreter library is reusable by an eventual application/API integration.

## Create the next 1,000 questions

1. Read the dataset card, requirements, annotation guide and previous report.
2. Keep round 01 frozen. Author `commercial-round-02/questions.jsonl` with
   exactly 1,000 distinct plausible commercial questions, decisions and typed
   expected IRs. Use the actual catalogue IDs and current core CNAE mappings.
3. Vary business intent and semantics, not only city/sector substitutions.
   Include ordinary prospecting, relative/explicit dates, minimal semantic
   pairs, multi-turn changes, unsupported filters, mixed requests, numeric
   boundaries, homonyms, unknown entities and out-of-domain requests.
4. Label independently of the current prediction. Review any suspected label
   defect against the source rule before changing it, recording the reason.
5. Run every question through `eval-questions`; compare all executable shapes
   against the actual local data. Record wrong IRs, refusals, clarification
   failures, execution errors and independent mismatches separately.
6. Fix general grammar/execution defects, add focused regressions, and rerun
   both the new round and the existing frozen suites. Keep failures visible.
7. Run the delivery gate and save commands, full raw output, hashes and a short
   report with limits. Human questions and human approval belong to the later
   product-validation stage, not to this synthetic iteration.

## Delivery gate

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Short prompt for another AI

Read AGENTS.md and docs/intelligence/results/commercial-round-01/{REPORT,RUNBOOK}.md.
Repeat the completed process for round 02: author 1,000 new synthetic commercial
questions with independent labels; run every text through eval-questions and the
real local data; fix general defects without changing labels to fit predictions;
rerun frozen suites and the delivery gate; save raw evidence and a concise report.
Keep H-blind intact, defer human validation to product refinement, and do not
commit or push.
