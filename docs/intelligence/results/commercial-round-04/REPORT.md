# Commercial research round 04 — new questions, same loop, measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

Round 04 contains exactly **5,000 new synthetic commercial questions**, in 20
categories of 250, using all 213 curated sectors and scenarios across all 27
UFs (250 municipalities). Every question, decision and typed expected IR was
authored independently of the interpreter's predictions: labels were fixed
only against source rules (round-01/02/03 phrasing precedents, catalogue data,
calendar arithmetic), never fitted to predictions. It was used in development;
it is not a blind or human validation set. No external question corpus, paid
service, generative runtime or Python dependency was added. Rounds 01–03 stay
frozen; rounds 01 and 02 still score 1,000/1,000 and round 03 scores
2,994/3,000 after the fixes below (replays in `/tmp`, not committed).

## National execution

- **4986/5,000** matches against this round's synthetic labels.
- **4475/4475 executable IRs** returned successfully; **0 invalid gold labels**.
- **4,225/4,225** labelled dialogue acts matched in the fifty five-turn sessions.
- **2,789 current aggregate/engine comparisons** matched exactly.
- **175 historical comparisons** matched raw marginal sums from the historical
  artifacts; 75 have no independent marginal reference (single-artifact
  results, retained as data outcomes).
- **1,186 requests** used a single engine path. They succeeded, but are not
  independent differential proofs.
- **541 zero scalar results** and **93 empty record searches** were retained as
  legitimate data outcomes.
- No backend error or differential mismatch was hidden, converted to success or
  dropped.

For executable questions, a match is equality of canonical typed requests,
including all explicit filters. For clarification/rejection cases it is the
expected decision, not a proof that every explanation or option is ideal.
Historical marginal checks use the same archived data through separate logic;
they are not a comparison with a separately available old engine snapshot.

The runner receives only text plus its own prior predictions. It never reads
`gold.ir` or `context.previous_ir` as interpreter input. Labels are used after
prediction for scoring and for a separate backend audit. Both predicted and
expected execution results are retained in `national/results.jsonl`.

Dataset SHA-256: `a92650ceda7328a4bd500f94497d00df4e79a82f5d2c37b2827f1e3f899141dd`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b` (same local
files as rounds 01–03). The municipality catalogue hash was checked as well.
The dataset manifest pins question, export and source-catalogue hashes.

## Examples from actual execution

- `commercial-r04-0001` — Para ampliar minha carteira: quantos estabelecimentos ativos de casa de festas existem na cidade de Maracanau/CE? Result: `{"scalar":37}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r04-2501` — Qual a densidade por 10 mil habitantes de loja de departamentos na cidade de Aguas Lindas de Goias/GO, usando estimativa IBGE 2026? Result: `{"denominator":249978,"numerator":6}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r04-3501` — Quantos estabelecimentos ativos de loja de roupas existem em AC, exceto os da cidade de Macapa/AP? Result: `{"scalar":3952}`. Backend: `aggregates`; reference: `engine_match`.

All units, date windows, catalogue versions and resolved CNAEs are retained in
the corresponding canonical requests. Record output includes only public CNPJ
identifiers and cadastral business fields; contacts are filtered by presence,
without copying names, partners, CPFs, phone numbers or e-mail addresses to the
result artifacts.

## Leftover analysis: the 14 mismatches

Thirteen are `place_typo` stress items and one is a noise-casualty trends item;
all other 4,986 match, including every new semantic shape in this round
(status-date month windows, strict capital/population bounds, national ranks,
three-UF comparisons, no-activity counts, deictic national scope, vague-quantity
and how-to/bolsa/concurso rejections, capital dialogue turns, sector-replacement
turns, per-10k comparisons, census ranks, secondary CNAE scope, class/division/
group codes, multi-status stock, MEI+Simples combinations, capital+natureza
combinations, YTD/recent windows, k=3/5/20 ranks, placeless lists with default
limit 20, all-caps and extra-punctuation noise).

- Twelve place-typo items clarify instead of resolving (`Valparaísoo`,
  `Maraabá`, `Guajaráá-Mirim`, `Duque dee Caxias`, `Cachoeiro dee Itapemirim`,
  `Ji-Paaraná`, `Rio VVerde`, `Senador GGuiomard`, `São Bernarddo do Campo`,
  `Ribeirão ddas Neves`, `Porto AAlegre`, `Ponta GGrossa`): multi-word city
  names and prefix-cities (`Vila Velha`, `Rio de Janeiro`, `Salvador`,
  `Ribeirão`, `Porto Alegre`) never reach the single-word-leaf typo recovery,
  by design.
- One place-typo item (`Águas Lindaas de Goiás`) resolves to two municipalities
  instead of one: the duplicated letter lands within edit distance of a second
  catalogue name, the documented ambiguity failure mode of the same bound.
- One trends item (net series for sawmills in Três Lagoas with a `light_typo`
  `estabelcimentos`): the typo collides with a numeral city name and both the
  place split and the sector drop out. Same mechanism as the round-03 twin;
  kept visible rather than engineered around, since numeral-bearing names are
  the known fragile corner.

The interpreter's bounded place-typo recovery therefore holds at 17/30 in this
round (14/20 in round 03): single-word leaf cities recover, prefix-cities and
multi-word cities do not. Widening recovery there risks correcting toward the
wrong municipality, so no code change was made and the bound stays documented
and measured.

## Cross-check against previously frozen suites

These inputs were not rewritten. They were run through actual text interpretation,
with the available archives declared, using the same canonical comparator.
Scores are byte-identical to the round-01 report; round 04 required no grammar
changes at all.

- **frozen-adv: 198/304** canonical/decision matches; 106 disagreements, 12 executions against a clarify/reject label. Raw files: `frozen-adv/`; stdout: `frozen-adv.log`.
- **frozen-informal: 116/202** canonical/decision matches; 86 disagreements, 2 executions against a clarify/reject label. Raw files: `frozen-informal/`; stdout: `frozen-informal.log`.
- **frozen-dialogues: 81/122** canonical/decision matches; 41 disagreements, 0 executions against a clarify/reject label. Raw files: `frozen-dialogues/`; stdout: `frozen-dialogues.log`.
- **frozen-ood: 377/400** canonical/decision matches; 23 disagreements, 0 executions against a clarify/reject label. Raw files: `frozen-ood/`; stdout: `frozen-ood.log`.

OOD produced **no executed requests**. The lower scores on ADV, informal text and
dialogues show that a perfect development-round score is **not** universal
Portuguese coverage or readiness for product validation. The gold labels must be
reconciled with explicit product rules during refinement, without changing them
simply to fit predictions.

## Changes driven by the evaluation

No interpreter code changes were needed for round 04: every new semantic shape
matched on the first full run except the documented noise leftovers above. The
round-03 grammar fixes (closure-count nouns, masculine secondary scope, plural
porte reset, lottery policy, per-flag MEI/Simples negation scoping, coordinated
exclusions) carried over without regression, covered by the focused tests in
`crates/opencnpj-nlu/src/tests.rs`.

Label defects found by review against source rules and corrected before the
final run: a double-shifted sector helper that misaligned gold sector IDs with
question texts (caught by a sector-in-text audit, all 5,000 re-verified);
zero-padded CNAE class/division/group codes; grammatical MEI+Simples predicates;
unit readings for head-office questions; comparison, rank, breakdown, capital,
trend, historical-rank, density, methodology, dialogue, closure and exclusion
phrasings aligned with supported constructions. First scored run was
1,335/5,000 (entirely the double-shift bug); after the one-line fix it was
4,981/5,000; the `lojas`-only aliquota labels were qualified and the set
regenerated to the final 4,986/5,000 with zero label-to-prediction fitting.

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` exited successfully. The raw test output records
**292 passed, 0 failed, 5 ignored**. See `fmt.log`, `clippy.log`, and `test.log`.

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

Keep all four rounds frozen, and collect human questions with human approval for
product validation. Prioritize the remaining frozen-suite disagreements
(unqualified place names, informal count/list readings, sector-vs-CNAE
representations, rank defaults, short references, pre-archive labels) and the
place-typo recovery bound documented above and in round 03. Preserve raw
failures and rerun all sets. Human validation belongs to product refinement,
not to synthetic iteration.

## Per-category evidence

- `activity`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `capital_legal`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `clarification`: 250/250 interpretation matches; 0 executable labels returned successfully.
- `closures`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `comparison`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `density`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `dialogue`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `distribution`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `exclusion`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `geography`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `historical`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `lookup`: 250/250 interpretation matches; 225 executable labels returned successfully.
- `market_size`: 237/250 interpretation matches; 250 executable labels returned successfully.
- `methodology`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `prospecting`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `registrations`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `structure_status`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `tax_size`: 250/250 interpretation matches; 250 executable labels returned successfully.
- `trends`: 249/250 interpretation matches; 250 executable labels returned successfully.
- `unsupported`: 250/250 interpretation matches; 0 executable labels returned successfully.
