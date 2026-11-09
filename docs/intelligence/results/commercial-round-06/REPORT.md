# Commercial research round 06 — 20,000 questions, same loop, measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

Round 06 contains exactly **20,000 new synthetic commercial questions** in 20
categories of 1,000: 17,900 executable requests, 1,000 clarifications and 1,100
rejections. It is built from two deterministic panels of 10,000 with different
catalogue index shifts, city orders, OOD slices, historical month pairs, rank
sizes and CNPJ roots. Every decision and typed expected IR was authored
independently of the interpreter's predictions: labels come from the catalogue
data, the source rules and calendar arithmetic, never from a prediction.

The first panel draft reused the round-05 shifts and produced too many
filler-only collisions, so it was regenerated with new shifts and roots. Two
hand-entered historical month labels (2026-01 and 2026-02) were initially one
competence too high; they were corrected against the calendar formula before
the final scoring. No label was changed to fit a prediction.

The 20,000 normalized texts are distinct. Overlap with rounds 01-05 is limited
to the documented exemptions: 213 dialogue turns 2-4 and four fixed small-talk
sentences (217 items). 19,002 items needed no filler repair; 998 distinct items
received a filler-only repair because a template collision remained (567
prior-round collisions, 476 cross-panel collisions, 208 within-panel
collisions; the sets overlap). Those repairs change only filler words, never a
slot, filter or gold IR. This is a synthetic development set, not a blind or
human validation set.

## National execution

- **19,954/20,000** matches against this round's synthetic labels (99.77 %).
- **17,900/17,900 executable IRs** returned successfully; **0 invalid gold
  labels** and **0 out-of-policy executions**.
- **16,900/16,900** labelled dialogue acts matched.
- **11,140 current aggregate/engine comparisons** matched exactly.
- **700 historical comparisons** matched independent marginal sums; **300**
  have no independent marginal reference (single-artifact results, retained as
  data outcomes); **1,000** methodology/meta results are non-numeric.
- **4,760 requests** used a single engine path. They succeeded, but are not
  independent differential proofs.
- **2,166 zero scalar results** and **454 empty record searches** were retained
  as legitimate data outcomes.
- No backend error, differential mismatch, wrong-answer execution or dropped
  row was hidden or converted to success.

For executable questions, a match is equality of canonical typed requests,
including all explicit filters. For clarification/rejection cases it is the
expected decision, not a proof that every explanation or option is ideal.
Historical marginal checks use the same archived data through separate logic;
they are not a comparison with a separately available old engine snapshot.

The runner receives only text plus its own prior predictions. It never reads
`gold.ir` or `context.previous_ir` as interpreter input. Labels are used after
prediction for scoring and for a separate backend audit. Both predicted and
expected execution results are retained in `national/results.jsonl`.

Dataset SHA-256:
`bf6770362c83b7095e345a6af913291e379eed0daa5f382521c15b8c80df40ff`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`
(same local files as rounds 01-05). The municipality catalogue hash was checked
as well. The dataset manifest pins question, export and source-catalogue hashes.

## Changes driven by the evaluation

All fixes are general interpreter fixes. No gold label was changed to fit a
prediction; the focused regressions live in `crates/opencnpj-nlu/src/tests.rs`.

- **Trailing punctuation after a CNPJ lookup.** The first scored pass had 128
  lookup mismatches caused by the filler repair form
  `.../0001-87, por favor?`: the comma stayed attached to the identifier and the
  lookup became a clarification. The parser now trims non-alphanumeric edges
  before validating the CNPJ. Lookup is 1,000/1,000 in the final run.
- **Sector/municipality homonym in an activity construction.** `serraria` is
  both a curated sector and the municipality Serraria/PB. In the trend phrase
  `saldo de registros menos baixas cadastrais de estabelecimentos de serraria`,
  the metric word `menos` made the parser treat the sector as a place and
  produced a wrong municipality set. An activity construction now prefers the
  sector; the geographic use `em Serraria/PB` still resolves the municipality.
- **Entity-aware intent cues.** `Itaporanga d'Ajuda/SE` contains the cue word
  `ajuda`, so a small-talk sentence was accepted as a help request. Cue matching
  for meta/help and the intent families now ignores words inside recognized
  entity spans. The same change fixes the known `Contagem` cue collision: the
  round-05 replay rises from 9,973 to 9,975 with no new failures.
- **Fail closed on multi-word place typos.** A probe found `Águas Lindaas de
  Goiás/GO` being accepted as two unrelated municipalities (`Aguaí/SP` and
  `Goiás/GO`) and `Porto SSeguro/BA` as `Porto/PI`. Fuzzy recovery now applies
  only to a complete single-token place followed by a UF and only when the
  candidate is unique; a multi-word place must be fully recognized up to its UF.
  The probe changed a wrong execution into a clarification. The place-typo bound
  and its leftovers are measured below.

## Leftover analysis: the 46 mismatches

All 46 remaining mismatches are `place_typo` stress items in `market_size`
(city name with one duplicated letter). Every one is predicted as `clarify`;
there is no wrong-answer execution. The 120 place-typo items split as follows:

- 74 recovered to the labelled municipality;
- 46 failed closed: 35 multi-token names and 11 single-token names whose
  duplicated-letter form has more than one one-edit catalogue candidate.

Examples of the retained leftovers:

- `Águas Lindaas de Goiás/GO` -> clarify (multi-token name);
- `Porto SSeguro/BA` -> clarify (multi-token name);
- `Lucas do RRio Verde/MT` -> clarify (multi-token name);
- `Santtos/SP` -> clarify (single token with an ambiguous fuzzy candidate).

The rule is deliberate: widening recovery to every token would have accepted
the wrong municipality in the probe above. The bound is documented and kept
visible; no code change was made to fit these labels. Human refinement must
decide whether to offer a disambiguation list for multi-word place typos. All
other 19,954 items match, including every new shape in this round (new
historical month pairs, rank sizes, CNPJ roots, panel-specific OOD slices,
dialogue sessions and the heavier noise slice).

## Cross-check against previously frozen suites

These inputs were not rewritten. They were run through actual text
interpretation with the same canonical comparator and declared archives.

- **frozen-adv: 198/304** canonical/decision matches; 106 disagreements, 12
  executions against a clarify/reject label.
- **frozen-informal: 116/202**; 86 disagreements, 2 executions against a
  clarify/reject label.
- **frozen-dialogues: 81/122**; 41 disagreements, 0 executions against a
  clarify/reject label.
- **frozen-ood: 377/400**; 23 disagreements, 0 executions against a
  clarify/reject label.

Earlier commercial rounds replayed with the final binary (`--interpret-only`,
outputs outside the repository): round 01 1000/1000, round 02 1000/1000, round
03 2994/3000, round 04 4987/5000, round 05 9975/10000. Round 04 gains one item
and round 05 gains two from the entity-aware cue fix; no prior-round score
regressed. The lower ADV, informal and dialogue scores still show that a strong
development-round score is **not** universal Portuguese coverage or readiness
for product validation.

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
warnings` and `cargo test --workspace` exited successfully. The raw test output
records **294 passed, 0 failed, 5 ignored**. See `fmt.log`, `clippy.log` and
`test.log`.

This is a working deterministic implementation and repeatable evaluation loop,
with measured development coverage. It does **not** complete every phase-4/5
architecture gate. Finite examples cannot prove every phrasing or filter
combination. The place-typo bound, the frozen-suite disagreements (unqualified
place names, informal count/list readings, sector-vs-CNAE representations, rank
defaults, short references, pre-archive labels), confidence calibration,
compiled language packs, signed HTTP context transport, application/API
integration and target-hardware measurements remain outside this delivery. The
CLI's state file is local state.

Human question collection and human evaluation are explicitly deferred to
product refinement, as requested. No human approval was required to execute this
synthetic round. H-blind was neither opened nor run. Existing source datasets
were not modified; snapshot format and production store code remain unchanged.
No commit or push was made.

## Per-category evidence

- `activity`: 1000/1000 interpretation matches; 1000 executable labels returned successfully.
- `capital_legal`: 1000/1000; 1000 executable labels returned successfully.
- `clarification`: 1000/1000; 0 executable labels returned successfully.
- `closures`: 1000/1000; 1000 executable labels returned successfully.
- `comparison`: 1000/1000; 1000 executable labels returned successfully.
- `density`: 1000/1000; 1000 executable labels returned successfully.
- `dialogue`: 1000/1000; 1000 executable labels returned successfully.
- `distribution`: 1000/1000; 1000 executable labels returned successfully.
- `exclusion`: 1000/1000; 1000 executable labels returned successfully.
- `geography`: 1000/1000; 1000 executable labels returned successfully.
- `historical`: 1000/1000; 1000 executable labels returned successfully.
- `lookup`: 1000/1000; 900 executable labels returned successfully (the 100 invalid check digits correctly clarify).
- `market_size`: 954/1000; 1000 executable labels returned successfully; the 46 mismatches are the place-typo leftovers above.
- `methodology`: 1000/1000; 1000 executable labels returned successfully.
- `prospecting`: 1000/1000; 1000 executable labels returned successfully.
- `registrations`: 1000/1000; 1000 executable labels returned successfully.
- `structure_status`: 1000/1000; 1000 executable labels returned successfully.
- `tax_size`: 1000/1000; 1000 executable labels returned successfully.
- `trends`: 1000/1000; 1000 executable labels returned successfully.
- `unsupported`: 1000/1000; 0 executable labels returned successfully.
