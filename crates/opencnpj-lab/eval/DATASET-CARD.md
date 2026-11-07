# Evaluation sets — ADV, OOD, informal and dialogues

First frozen on 2026-11-06 by a generator script that was removed on 2026-11-07: the
JSONL files are now the source of truth and are edited by hand. Gold IRs are labelled
against the committed catalogues (`opencnpj-insights/catalog/municipalities.tsv` and
`sectors.toml`, core CNAE sets); the frozen-set test validates every gold against them.
After editing a file, refresh the rows below with
`UPDATE_CARD=1 cargo test -p opencnpj-lab frozen_sets`.

Status: AI-drafted. These sets do not meet WP-3.3, which requires sets written by people
who never saw the generator or the grammar. They must not be used for the WP-5.1
decision. See results/2026-11-07-review.md.

| File | Items | SHA-256 |
|---|---|---|
| adv.jsonl | 235 | f064dd7a6e1367732d16c217bf6b1a862dfd5750b4d1719da12fd26971e31e12 |
| ood.jsonl | 400 | c3eafa0206e53cd908de5ee71d81a5f7a6de4015f35c11b6036af207f158eb92 |
| h-informal.jsonl | 80 | f908c37a78b08f818cf3d0c2d629e9104487cf4c99ec552c220c97e21eec0fce |
| dialogues.jsonl | 48 | 3b1d89e24d308120decbdac9177a7113417a3484f1083efe18b7f63b4fcbb1b5 |

## Schemas

* `adv.jsonl`, `ood.jsonl`, `h-informal.jsonl`: one `opencnpj_query::ir::Request`
  gold per executable item, or a clarify/reject decision with a rationale.
* `dialogues.jsonl`: `{"id","turns":[{"text","gold"}...],"meta"}`; the gold
  after each turn is a complete IR or a decision.

## Coverage

Traps of 05-INTERPRETER.md §7: abertas=96, crescendo=25, fecharam=25, grandes=4, mais=2, menos=2.
Semantic rules with at least five items: SR-01=80, SR-02=96, SR-03=16, SR-04=25, SR-05=25, SR-06=18, SR-07=9, SR-08=16, SR-09=9, SR-10=10, SR-11=5, SR-12=4, SR-13=13, SR-14=5.

## Sources and attribution

The OOD set is original for this evaluation. The planned import of
`OHF-Voice/intents` pt-BR (CC BY 4.0) was attempted through the Hugging Face
datasets server on 2026-11-06; the dataset is gated and the request returned an
authentication error, so **no rows were imported**. If the import is retried,
retain the CC BY 4.0 attribution and add the rows to a separate section of this
card; do not mix them into H-blind.

## Usage

`opencnpj-intel eval --set eval/adv.jsonl --predictions ...` (or the baseline)
and the schema test in `opencnpj-lab`. The H-blind guard does not apply to these
sets.
