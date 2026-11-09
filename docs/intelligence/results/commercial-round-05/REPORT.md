# Commercial research round 05 — new questions, same loop, measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

Round 05 contains exactly **10,000 new synthetic commercial questions**, in 20
categories of 500, using all 213 curated sectors and scenarios across all 27
UFs (250 municipalities). Every question, decision and typed expected IR was
authored independently of the interpreter's predictions: labels were fixed
only against source rules (round-01/02/03/04 phrasing precedents, catalogue data,
calendar arithmetic), never fitted to predictions. Scale-up text repairs
(`dedup_filler`, `overlap_repair`) change only filler words, never slots or
gold IRs. It was used in development; it is not a blind or human validation
set. No external question corpus, paid service, generative runtime or Python
dependency was added. Rounds 01–04 stay frozen; rounds 01 and 02 still score
1,000/1,000, round 03 scores 2,994/3,000 and round 04 scores 4,986/5,000 after
the fix below (replays in `/tmp`, not committed).

## National execution

- **9973/10,000** matches against this round's synthetic labels.
- **8950/8950 executable IRs** returned successfully; **0 invalid gold labels**.
- **8,449/8,450** labelled dialogue acts matched (the single act mismatch is
  the Contagem turn below, predicted `new_query` instead of `replace_slot`).
- **5,570 current aggregate/engine comparisons** matched exactly.
- **350 historical comparisons** matched raw marginal sums from the historical
  artifacts; 150 have no independent marginal reference (single-artifact
  results, retained as data outcomes).
- **2,380 requests** used a single engine path. They succeeded, but are not
  independent differential proofs.
- **1,093 zero scalar results** and **338 empty record searches** were retained
  as legitimate data outcomes.
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

Dataset SHA-256: `113ad5d2b44bfcf4a90e33bda83612a000fde7b0816c4403fc005a7af1c3a09b`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b` (same local
files as rounds 01–04). The municipality catalogue hash was checked as well.
The dataset manifest pins question, export and source-catalogue hashes.

## Examples from actual execution

- `commercial-r05-00001` — Para expandir minha operacao: quantos estabelecimentos ativos de loja de colchoes existem na cidade de Vitoria da Conquista/BA? Result: `{"scalar":49}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r05-02501` — Quantos estabelecimentos de servico de paisagismo foram registrados com inicio de atividade em 2025 na cidade de Itacoatiara/AM? Result: `{"scalar":5}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r05-03001` — Quantos estabelecimentos de serralheria tiveram baixa cadastral com inicio de atividade em 2025 na cidade de Barueri/SP? Result: `{"scalar":8}`. Backend: `aggregates`; reference: `engine_match`.

All units, date windows, catalogue versions and resolved CNAEs are retained in
the corresponding canonical requests. Record output includes only public CNPJ
identifiers and cadastral business fields; contacts are filtered by presence,
without copying names, partners, CPFs, phone numbers or e-mail addresses to the
result artifacts.

## Leftover analysis: the 27 mismatches

Twenty-five are `place_typo` stress items in `market_size` (city name with one
duplicated letter), resolving to clarify instead of the labelled municipality —
the documented bounded place-typo recovery limit (single-word leaf cities
recover; prefix-cities and multi-word cities do not, by design, since widening
recovery risks correcting toward the wrong municipality). No code change was
made for these; the bound stays documented and measured.

The remaining two are turns 3–4 of dialogue session `commercial-session-159`
(`commercial-r05-08793`, `commercial-r05-08794`): `Só matrizes na cidade de
Contagem/MG.` / `Inclua filiais também na cidade de Contagem/MG.` The city name
Contagem normalizes to the count cue `contagem`, so the elliptical follow-up is
read as a new query and the inherited sector is dropped (turn 3 also mismatches
its dialogue act). Only Contagem and Registro collide with cue words among all
5,572 catalogue municipalities, and only `contagem` affects follow-up detection;
all other 98 sessions' city-qualified turns match. Entity-aware cue matching
would be the principled fix, but it risks regressions across the grammar for a
two-turn corner in the same family as the documented numeral-city fragility, so
no code change was made and the failure is kept visible. Future synthetic
rounds should exclude Contagem from dialogue city-qualified slots; human
refinement must decide the product rule for cue-colliding city names.

All other 9,973 match, including every new shape in this round (doubled
per-category coverage, 100 five-turn sessions, strict bounds, national ranks,
three-UF and per-10k comparisons, no-activity counts, sector-replacement and
capital dialogue turns, heavier noise with dedup/overlap filler repairs).

## Cross-check against previously frozen suites

These inputs were not rewritten. They were run through actual text interpretation,
with the available archives declared, using the same canonical comparator.
Scores are byte-identical to the round-01 report; the grammar fix below caused
no regression there.

- **frozen-adv: 198/304** canonical/decision matches; 106 disagreements, 12 executions against a clarify/reject label. Raw files: `frozen-adv/`; stdout: `frozen-adv.log`.
- **frozen-informal: 116/202** canonical/decision matches; 86 disagreements, 2 executions against a clarify/reject label. Raw files: `frozen-informal/`; stdout: `frozen-informal.log`.
- **frozen-dialogues: 81/122** canonical/decision matches; 41 disagreements, 0 executions against a clarify/reject label. Raw files: `frozen-dialogues/`; stdout: `frozen-dialogues.log`.
- **frozen-ood: 377/400** canonical/decision matches; 23 disagreements, 0 executions against a clarify/reject label. Raw files: `frozen-ood/`; stdout: `frozen-ood.log`.

OOD produced **no executed requests**. The lower scores on ADV, informal text and
dialogues show that a strong development-round score is **not** universal
Portuguese coverage or readiness for product validation. The gold labels must be
reconciled with explicit product rules during refinement, without changing them
simply to fit predictions.

## Changes driven by the evaluation

General interpreter defect, fixed without touching any label to fit a prediction
(focused regression in `crates/opencnpj-nlu/src/tests.rs`):

- Explicit CNAE division/group/class codes now tolerate trailing filler
  punctuation (`divisão 43, por favor?`, `grupo 474, por favor?`). The parser
  stripped only whole-word digits, so the round-05 dedup filler comma dropped
  two activity filters to `any`. First scored run was 9,971/10,000; after the
  fix it is 9,973/10,000 with zero label-to-prediction fitting.

Label defects found by review against source rules and corrected before the
final run: none required correction beyond the script's own sector-in-text and
overlap audits — every gold IR validated against the committed catalogues, all
10,000 normalized texts distinct, and no folded-text overlap with rounds 01–04
(except exempt dialogue turns 2–4 and four fixed small-talk sentences).

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` exited successfully. The raw test output records
**293 passed, 0 failed, 5 ignored**. See `fmt.log`, `clippy.log`, and `test.log`.

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

Keep all five rounds frozen, and collect human questions with human approval for
product validation. Prioritize the remaining frozen-suite disagreements
(unqualified place names, informal count/list readings, sector-vs-CNAE
representations, rank defaults, short references, pre-archive labels), the
place-typo recovery bound, and the Contagem/cue collision documented above.
Preserve raw failures and rerun all sets. Human validation belongs to product
refinement, not to synthetic iteration.

## Per-category evidence

- `activity`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `capital_legal`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `clarification`: 500/500 interpretation matches; 0 executable labels returned successfully.
- `closures`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `comparison`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `density`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `dialogue`: 498/500 interpretation matches; 500 executable labels returned successfully.
- `distribution`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `exclusion`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `geography`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `historical`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `lookup`: 500/500 interpretation matches; 450 executable labels returned successfully.
- `market_size`: 475/500 interpretation matches; 500 executable labels returned successfully.
- `methodology`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `prospecting`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `registrations`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `structure_status`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `tax_size`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `trends`: 500/500 interpretation matches; 500 executable labels returned successfully.
- `unsupported`: 500/500 interpretation matches; 0 executable labels returned successfully.
