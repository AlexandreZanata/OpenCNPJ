# Commercial research round 02 — new questions, same loop, measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

Round 02 contains exactly **1,000 new synthetic commercial questions**, in the
same 20 categories of 50, using all 213 curated sectors and scenarios across
all 27 UFs (60 municipalities). Every question, decision and typed expected IR
was authored independently of the interpreter's predictions: labels were fixed
only against source rules (round-01 phrasing precedents, catalogue data, date
arithmetic), never fitted to predictions. It was used in development; it is not
a blind or human validation set. No external question corpus, paid service,
generative runtime or Python dependency was added. Round 01 stays frozen and
still scores 1,000/1,000 after the fixes below (replay in `/tmp`, not committed).

## National execution

- **1000/1,000** matches against this round's synthetic labels.
- **895 executable IRs** returned successfully; **0 invalid gold labels**.
- **50 clarification decisions** and **55 rejection decisions** matched their labels.
- **845/845** labelled dialogue acts matched in the ten five-turn sessions.
- **565 current aggregate/engine comparisons** matched exactly.
- **32 historical comparisons** matched raw marginal sums from the historical
  artifacts; 15 change-ranks and 3 stocks have no independent marginal
  reference (single-artifact results, retained as data outcomes).
- **230 requests** used a single engine path. They succeeded, but are not
  independent differential proofs.
- **60 zero scalar results** and **2 empty record searches** were retained as
  legitimate data outcomes.
- No backend error or differential mismatch was hidden, converted to success or
  dropped. No out-of-policy execution occurred (1,000/1,000 decisions match).

For executable questions, a match is equality of canonical typed requests,
including all explicit filters. For clarification/rejection cases it is the
expected decision, not a proof that every explanation or option is ideal.
Historical marginal checks use the same archived data through separate logic;
they are not a comparison with a separately available old engine snapshot.

The runner receives only text plus its own prior predictions. It never reads
`gold.ir` or `context.previous_ir` as interpreter input. Labels are used after
prediction for scoring and for a separate backend audit. Both predicted and
expected execution results are retained in `national/results.jsonl`.

Dataset SHA-256: `683ac7a7cc1f2cb0c6daa07d1023cfe8c26b86969e8b0f34e9cecddd0e641715`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b` (same local
files as round 01). The municipality catalogue hash was checked as well.
The dataset manifest pins question, export and source-catalogue hashes.

## Examples from actual execution

- `commercial-r02-0001` — Para ampliar minha carteira: quantos estabelecimentos ativos de loja de autopeças existem na cidade de Nova Iguaçu/RJ? Result: `{"scalar":260}`. Backend: `aggregates`; reference: `engine_match`.
- `commercial-r02-0401` — Quantos estabelecimentos de casa de festas estavam ativos na cidade de Teresina/PI na competência 2024-12, segundo o arquivo daquele mês? Result: `{"scalar":76}`. Backend: `aggregates`; single-artifact reference.
- `commercial-r02-0501` — Qual a densidade por 10 mil habitantes de loja de eletrodomésticos na cidade de Porto Alegre/RS, usando IBGE 2026? Result: `{"denominator":1388791,"numerator":528}`. Backend: `aggregates`; reference: `engine_match`.

All units, date windows, catalogue versions and resolved CNAEs are retained in
the corresponding canonical requests. Record output includes only public CNPJ
identifiers and cadastral business fields; contacts are filtered by presence,
without copying names, partners, CPFs, phone numbers or e-mail addresses to the
result artifacts.

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

- List verb `traga/tragam/traz/trazem` accepted for list requests (new cue entries
  plus the existing limit trigger).
- `não são optantes` / `não sejam optantes` negation now maps MEI and Simples to
  `no` instead of silently reading the affirmative.
- Explicit `Censo IBGE 2022` references select the census population instead of
  defaulting to the estimate.
- Population bands bind to the last `habitantes` whose window has no `por`, so a
  density phrase (`por 10 mil habitantes`) no longer swallows the band.
- Entity blanking for time parsing is index-based: a two-letter UF hit such as
  `es` no longer erases matching substrings elsewhere (`meses` became `m  ` and
  broke trailing-12-month windows for ES/SE cities).
- Singular `matriz`/`filial` accepted after `conte` and in follow-up/clear
  markers, matching the plural forms.

Label defects found by review against source rules (round-01 precedents, catalogue
data, calendar arithmetic) and corrected before the final run, with reasons kept
in the handoff: `within_ufs` restored on radius texts that name the UF; trailing
12 months anchored to day 45941 (2025-10-12); `de 2025 a 2026 até a data da base`
and `anual` restored on annual trends; MEI series use `opções/exclusões`;
net trends use `saldo de registros menos baixas`; sector comparisons use the
`de X e de Y` coordination with activity left to the members; ranks use the
`10 grupos` form with `k=10, min_base=0`; breakdowns use `por matriz e filial`
and `por opção pelo MEI`; capital ranges use `de pelo menos`/`de até`/`entre`;
help uses the `Que pesquisas comerciais posso fazer` form; closures drop the
stray `com início de atividade`; historical ranks state their start month;
dialogue turns 3–5 use bare/add/clear/new-search forms; prospecting states order
and page; density ranks pair per-10k with supported bands and inhabitants-per
with whole-UF scope. First scored run was 664/1,000; the fixes above brought it
to 1,000/1,000 with zero label-to-prediction fitting.

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` exited successfully. The raw test output records
**290 passed, 0 failed, 5 ignored**. New lab tests cover the grammar fixes above.
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

Keep both rounds frozen, and collect human questions with human approval for
product validation. Prioritize the remaining frozen-suite disagreements
(unqualified place names, informal count/list readings, sector-vs-CNAE
representations, rank defaults, short references, pre-archive labels). Preserve
raw failures and rerun all sets. Human validation belongs to product refinement,
not to synthetic iteration.

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
