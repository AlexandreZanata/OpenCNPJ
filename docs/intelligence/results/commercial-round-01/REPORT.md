# Commercial research round 01 — implemented interpreter and measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

The empty `opencnpj-nlu` modules now contain a deterministic interpreter and
explicit dialogue state. The new `opencnpj-intel ask` command interprets and
optionally executes a question. `eval-questions` applies every text, compares
canonical IRs and preserves per-item decisions and execution results.

Round 01 contains exactly **1,000 synthetic commercial questions**, in 20 groups,
using all 213 curated sectors and scenarios across all 27 UFs. It was used in
development; it is not a blind or human validation set. No external question
corpus, paid service, generative runtime or Python dependency was added.
The existing RFB/IBGE-derived catalogues supplied real vocabulary and identifiers.

## National execution

- **1000/1,000** matches against this round's synthetic labels.
- **895 executable IRs**: **845 data requests** and **50 static methodology/help responses** returned successfully.
- **50 clarification decisions** and **55 rejection decisions** matched their labels.
- **50/50** labelled dialogue acts matched in the ten five-turn sessions.
- **505 current aggregate/engine comparisons** matched exactly.
- **50 historical comparisons** matched raw marginal sums from the historical artifacts.
- **290 requests** used a single engine path. They succeeded, but are not independent differential proofs.
- **53 zero scalar results** and **8 empty record searches** were retained as legitimate data outcomes.
- No backend error or differential mismatch was hidden, converted to success or dropped.

For executable questions, a match is equality of canonical typed requests,
including all explicit filters. For clarification/rejection cases it is the
expected decision, not a proof that every explanation or option is ideal.
Historical marginal checks use the same archived data through separate logic;
they are not a comparison with a separately available old engine snapshot.

The runner receives only text plus its own prior predictions. It never reads
`gold.ir` or `context.previous_ir` as interpreter input. Labels are used after
prediction for scoring and for a separate backend audit. Both predicted and
expected execution results are retained in `national/results.jsonl`.

Dataset SHA-256: `65d962712b8e9a659bebe58da0b56ce9cbbd0b25d0b428ebb89e0e4f55bf4867`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`. The municipality catalogue hash was checked as well.
The dataset manifest pins question, export and source-catalogue hashes.

## Examples from actual execution

- `commercial-r01-0001` — qtos estabelcimentos ativos de escritorio de contabilidade existem na cidade de rio branco/ac? Result: `{"scalar":187}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r01-0253` — Quantos estabelecimentos de loja de pneus foram registrados com início de atividade em 2025 na cidade de Manaus/AM, e continuam ativos na base atual? Result: `{"scalar":12}`. Backend: `engine`; reference: `single_backend`.
- `commercial-r01-0401` — Quantos estabelecimentos de loja de artesanato estavam ativos em AC na competência 2024-12, segundo o arquivo daquele mês? Result: `{"scalar":147}`. Backend: `aggregates`; reference: `marginal_match`.

All units, date windows, catalogue versions and resolved CNAEs are retained in
the corresponding canonical requests. Record output includes only public CNPJ
identifiers and cadastral business fields; contacts are filtered by presence,
without copying names, partners, CPFs, phone numbers or e-mail addresses to the
result artifacts.

## Cross-check against previously frozen suites

These inputs were not rewritten. They were run through actual text interpretation,
with the available archives declared, using the same canonical comparator.

- **frozen-adv: 198/304** canonical/decision matches; 106 disagreements, 12 executions against a clarify/reject label. Raw files: `frozen-adv/`; stdout: `frozen-adv.log`.
- **frozen-informal: 116/202** canonical/decision matches; 86 disagreements, 2 executions against a clarify/reject label. Raw files: `frozen-informal/`; stdout: `frozen-informal.log`.
- **frozen-dialogues: 81/122** canonical/decision matches; 41 disagreements, 0 executions against a clarify/reject label. Raw files: `frozen-dialogues/`; stdout: `frozen-dialogues.log`.
- **frozen-ood: 377/400** canonical/decision matches; 23 disagreements, 0 executions against a clarify/reject label. Raw files: `frozen-ood/`; stdout: `frozen-ood.log`.

OOD produced **no executed requests**. Its remaining disagreements are refusals
versus clarification policy. The lower scores on ADV, informal text and dialogues
show that a perfect development-round score is **not** universal Portuguese
coverage or readiness for product validation. These suites also influenced some
subsequent general fixes, so they are not an untouched final test split either.

Disagreements include unqualified state/city names such as São Paulo and Rio de
Janeiro, count-versus-list readings of informal prompts, catalogue-sector versus
explicit-CNAE representations, minimum-base/ranking defaults, short dialogue
references, and historical questions whose older label assumes no archive.
For example, an older item labels 2025 active stock as clarification even though
the supplied archive can answer it. Such disagreements remain visible; this
report does not automatically declare either every old label or every prediction
correct. The gold labels must be reconciled with explicit product rules during
refinement, without changing them simply to fit predictions.

## Changes driven by the evaluation

- Longest catalogue matches, homonym handling and bounded place typo recovery;
  count/sector words are not automatically municipality names.
- Entity spans are separated from the time grammar: Janeiro in Rio de Janeiro
  cannot turn a requested full year into January.
- Main/secondary CNAE, tax/size/status/head-office filters, exclusions, legal
  nature, exact integer-cent currency ranges, population and geography rules.
- Lists, ordering, pagination, CNPJ/root lookup, comparisons, ranks, breakdowns,
  registration/closure/MEI series, historical stock/change and methodology.
- New-query reset, filter addition/replacement/removal, clarification options,
  host-supplied result references and out-of-domain/privacy handling.
- Exclusions and population conditions are applied before list ordering and
  pagination; rows with an unknown municipality are preserved when excluding
  only known municipalities.
- The engine oracle preserves an explicit same-flow status filter. It no longer
  turns “registered in 2025 and still active” into registrations in all statuses.
  A fixture distinguishes these populations and checks the count through a
  separate stock/date-filter path.
- Flow availability now matches the source date domain, instead of an arbitrary
  2025 start. Pre-2021/unaligned periods route to the engine as required; access
  to a historical stock snapshot remains a separate capability.

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` exited successfully. The raw test output records
**289 passed, 0 failed, 5 ignored**. The interpreter has 28 focused tests;
new lab tests cover frozen dataset integrity, complete list filtering and
same-flow status preservation. See `fmt.log`, `clippy.log`, and `test.log`.

This is a working deterministic implementation and repeatable evaluation loop,
with measured development coverage. It does **not** complete every phase-4/5
architecture gate. Finite examples cannot prove every phrasing or filter
combination. Confidence is not calibrated; compiled language packs, signed HTTP
context transport, application/API integration and target-hardware measurements
remain outside this delivery. The CLI's state file is local state.

Human question collection and human evaluation are explicitly deferred to product
refinement, as requested. No human approval was required to execute this synthetic
round. H-blind was neither opened nor run. Existing source datasets were not
modified; snapshot format and production store code remain unchanged. No commit
or push was made.

## Next iteration

Read [RUNBOOK.md](RUNBOOK.md), keep this round frozen, and author a genuinely new
1,000-question round with independent labels. Prioritize actual disagreements,
semantic minimal pairs and mixed conditions; do not inflate coverage by changing
only place/sector names. Preserve raw failures and rerun both new and old sets.
The short handoff prompt is at the end of the runbook.

## Per-category evidence

- `activity`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `capital_legal`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `clarification`: 50/50 interpretation matches; 0 executable labels returned successfully.
- `closures`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `comparison`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `density`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `dialogue`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `distribution`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `exclusion`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `geography`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `historical`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `lookup`: 50/50 interpretation matches; 45 executable labels returned successfully.
- `market_size`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `methodology`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `prospecting`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `registrations`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `structure_status`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `tax_size`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `trends`: 50/50 interpretation matches; 50 executable labels returned successfully.
- `unsupported`: 50/50 interpretation matches; 0 executable labels returned successfully.
