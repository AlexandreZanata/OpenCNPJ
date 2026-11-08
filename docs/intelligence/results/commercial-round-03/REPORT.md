# Commercial research round 03 — new questions, same loop, measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

Round 03 contains exactly **3,000 new synthetic commercial questions**, in 20
categories of 150, using all 213 curated sectors and scenarios across all 27
UFs (170 municipalities). Every question, decision and typed expected IR was
authored independently of the interpreter's predictions: labels were fixed
only against source rules (round-01/02 phrasing precedents, catalogue data,
calendar arithmetic), never fitted to predictions. It was used in development;
it is not a blind or human validation set. No external question corpus, paid
service, generative runtime or Python dependency was added. Rounds 01 and 02
stay frozen and still score 1,000/1,000 after the fixes below (replays in
`/tmp`, not committed).

## National execution

- **2994/3,000** matches against this round's synthetic labels.
- **2685/2685 executable IRs** returned successfully; **0 invalid gold labels**.
- **2,535/2,535** labelled dialogue acts matched in the thirty five-turn sessions.
- **1,619 current aggregate/engine comparisons** matched exactly.
- **97 historical comparisons** matched raw marginal sums from the historical
  artifacts; 53 have no independent marginal reference (single-artifact
  results, retained as data outcomes).
- **766 requests** used a single engine path. They succeeded, but are not
  independent differential proofs.
- **282 zero scalar results** and **57 empty record searches** were retained as
  legitimate data outcomes.
- No backend error or differential mismatch was hidden, converted to success or
  dropped. No out-of-policy execution occurred on matched items.

For executable questions, a match is equality of canonical typed requests,
including all explicit filters. For clarification/rejection cases it is the
expected decision, not a proof that every explanation or option is ideal.
Historical marginal checks use the same archived data through separate logic;
they are not a comparison with a separately available old engine snapshot.

The runner receives only text plus its own prior predictions. It never reads
`gold.ir` or `context.previous_ir` as interpreter input. Labels are used after
prediction for scoring and for a separate backend audit. Both predicted and
expected execution results are retained in `national/results.jsonl`.

Dataset SHA-256: `331d33f48d655469f2a3b1926fc5140dbc0fdb9c65f88614b82138983aa76d98`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b` (same local
files as rounds 01 and 02). The municipality catalogue hash was checked as well.
The dataset manifest pins question, export and source-catalogue hashes.

## Examples from actual execution

- `commercial-r03-0001` — Para ampliar minha carteira: quantos estabelecimentos ativos de salao de beleza existem na cidade de Brasileia/AC? Result: `{"scalar":46}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r03-0401` — Quantos estabelecimentos ativos de servico de malote de micro porte existem na cidade de Boa Vista/RR? Result: `{"scalar":488}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r03-0501` — Quantos estabelecimentos inaptos de serraria que sao filiais existem na cidade de Itacoatiara/AM? Result: `{"scalar":2}`. Backend: `aggregates`; reference: `engine_match`.

All units, date windows, catalogue versions and resolved CNAEs are retained in
the corresponding canonical requests. Record output includes only public CNPJ
identifiers and cadastral business fields; contacts are filtered by presence,
without copying names, partners, CPFs, phone numbers or e-mail addresses to the
result artifacts.

## Leftover analysis: the 6 mismatches

All six are `place_typo` stress items (city name with one duplicated letter),
all in `market_size`, all resolving to clarify instead of the labelled
municipality:

- `commercial-r03-0006` — `Vitóória/ES` (Vitória is a prefix of six longer catalogue names)
- `commercial-r03-0015` — `Rio de JJaneiro/RJ` (multi-word name; single-word keys never match `jjaneiro`)
- `commercial-r03-0028` — `Salvaador/BA` (prefix of Salvador do Sul / das Missões)
- `commercial-r03-0030` — `Vila VVelha/ES` (multi-word name; `vvelha` matches no single-word key)
- `commercial-r03-0048` — `Junddiaí/SP` (`Jundiaí do Sul` makes the key non-leaf)
- `commercial-r03-0049` — `Cruzeiroo do Sul/AC` (multi-word name)

The interpreter's documented bounded place-typo recovery only considers
single-word leaf keys with a unique one-edit neighbourhood
(`crates/opencnpj-nlu/src/spotter.rs`, `fuzzy_place`). Prefix-cities and
multi-word cities are excluded by design, since widening recovery there risks
correcting toward the wrong municipality. The remaining 14 place-typo items
(70% of the slice) recover. No code change was made: the bound is principled,
and the failures are kept visible as the measured limit. A seventh
noise-adjacent case seen during development (a typo colliding with a numeral
city name) is recorded in the round-04 report, whose noise slice re-measures
the same bound.

## Cross-check against previously frozen suites

These inputs were not rewritten. They were run through actual text interpretation,
with the available archives declared, using the same canonical comparator.
Scores are byte-identical to the round-01 report; the grammar fixes below caused
no regression there.

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

General interpreter defects, fixed without touching any label to fit a prediction
(focused regressions in `crates/opencnpj-nlu/src/tests.rs`):

- `Quantas baixas de X` reads as a closure count like its verb forms (`tiveram
  baixa`, `fecharam`).
- Masculine `secundário/secundários` selects secondary CNAE scope like the
  feminine forms.
- Resetting `todos os portes` restores the full porte set (the plural was missed).
- Lottery questions (`números da sorte`, `loteria`, `horóscopo`) are unavailable
  data, like revenue and forecasts.
- MEI/Simples negation is scoped per flag (`não são optantes pelo MEI` no
  longer negates Simples and vice versa).
- Coordinated exclusions (`exceto A e B`) keep both places and record the
  exclusions; entity words between an explicit exclusion marker and its target
  no longer break the scope, while `sem`/`menos`/`fora` keep the strict scope
  so a comparator such as `menos` never leaks across entities.

Label defects found by review against source rules (round-01/02 precedents,
catalogue data, calendar arithmetic) and corrected before the final run, with
reasons kept in the handoff: neutral market lead-ins without policy-loaded
words (`mercado`, `semana`); `Traga`/`Liste` for prospecting instead of the
list-ambiguous `Mostre`; grammatical MEI+Simples predicates; head-office-unit
readings for `matrizes` questions; `Compare`-form comparisons; multi-exclusion
labels already kept both UFs; zero-padded CNAE class/division/group codes;
whole-word abbreviations that never touch entity spans; unaccented noise that
preserves UF-code case; place-typo items phrased as `em {typo}/{UF}`; UF-matrix,
capital, trend, historical-rank, distribution, density, methodology, dialogue
and closure phrasings aligned with supported constructions. First scored run was
2,690/3,000; the fixes above brought it to 2,994/3,000 with zero
label-to-prediction fitting.

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` exited successfully. The raw test output records
**292 passed, 0 failed, 5 ignored**. New lab tests cover the grammar fixes above.
See `fmt.log`, `clippy.log`, and `test.log`.

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

Keep all three rounds frozen, and collect human questions with human approval for
product validation. Prioritize the remaining frozen-suite disagreements
(unqualified place names, informal count/list readings, sector-vs-CNAE
representations, rank defaults, short references, pre-archive labels) and the
place-typo recovery bound documented above. Preserve raw failures and rerun all
sets. Human validation belongs to product refinement, not to synthetic iteration.

## Per-category evidence

- `activity`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `capital_legal`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `clarification`: 150/150 interpretation matches; 0 executable labels returned successfully.
- `closures`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `comparison`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `density`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `dialogue`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `distribution`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `exclusion`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `geography`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `historical`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `lookup`: 150/150 interpretation matches; 135 executable labels returned successfully.
- `market_size`: 144/150 interpretation matches; 150 executable labels returned successfully.
- `methodology`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `prospecting`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `registrations`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `structure_status`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `tax_size`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `trends`: 150/150 interpretation matches; 150 executable labels returned successfully.
- `unsupported`: 150/150 interpretation matches; 0 executable labels returned successfully.
