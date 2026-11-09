# Commercial research questions — round 01

This is a **synthetic development set**, authored by an AI for the commercial
research workflows in `docs/intelligence/01-REQUIREMENTS.md`. These are plausible
customer questions, not collected customer conversations or human annotations.
Human evaluation is deferred to product refinement, as requested by the owner.
No human approval is required to execute this synthetic development round.

## Composition

Exactly 1,000 UTF-8 question records: 895 executable requests, 50 clarifications
and 55 rejections. There are 20 equally weighted categories of 50 questions:
market sizing, prospecting, tax/size, structure/status, capital/legal nature,
registrations, closures, trends, historical stock/growth, geography, density,
comparisons, distributions/rankings, activity/CNAE, exclusions, lookup,
methodology, dialogue, clarification, and unsupported requests.

The executable labels use all 213 current curated sector IDs. The geographic
scenarios include all 27 UFs and 50 municipalities, including the state capitals
and Boa Esperança do Norte. Questions include main/secondary CNAE, all operation
variants, all status/porte classes, MEI and Simples, headquarters/branches,
capital and legal nature, registration/closure dates, geography/population,
rank direction, list ordering/pagination and contact-presence filters.

There are ten five-turn sessions. `context.session_id` groups them;
`previous_ir` is an annotation for inspection, **never an interpreter input**.
The runner carries only the interpreter's own state between turns. Ten new
subject turns check that old filters do not leak into a new search.

58 questions have accent removal/case variation; 26 have a controlled spelling
error or abbreviation. These tags may overlap. All 1,000 normalized question
texts are distinct. Some executable labels are deliberately shared by
paraphrases; do not count these as independent business intents.

## Format and provenance

- `questions.jsonl` is the frozen, directly editable source of truth.
- `questions.tsv` is a reading/export view, with one row per question.
- `manifest.json` pins hashes of the questions and source catalogues.
- Every row has an ID, question, expected decision, expected typed IR when
  executable, rule tags, category, rationale and explicit synthetic authorship.
- Language is pt-BR. The data reference is **2026-10-11**, competence **2026-10**.
- Monetary IR values are integer cents. Population bounds and dates are inclusive.
- Sector IDs follow the checked-in `sectors.toml` file order, not display names.

Original scenario formulations and catalogue substitutions were assembled with a
one-off local authoring script. No third-party question dataset was imported.
The checked-in RFB/IBGE-derived catalogues supply vocabulary and identifiers;
their existing source manifests and licences remain authoritative. There is no
new Python dependency in the repository, interpreter, evaluator or replay flow.

## Label review before freezing

The authoring review corrected an invalid CNPJ's expected decision to clarify
(five formulations), changed unavailable 2010 stock requests to reject with
alternatives (five formulations), and removed an incidental sector from the
motivation of sector-ranking questions. These are semantic label corrections,
not adjustments to make erroneous predictions pass. They are separate from
interpreter iterations. Existing ADV, Informal, OOD, dialogue and human/blind
sets were not edited.

The fixture test checks size, IDs, normalized text uniqueness, category balance,
synthetic provenance and validation of every executable label. A separate
review should still challenge these AI-authored labels during product refinement.

## Evaluation contract

`opencnpj-intel eval-questions` receives each **question text** and its own
conversation state. The gold request is used only after prediction, for
canonical comparison and a separate execution audit. Predicted and gold IRs are
canonicalized; sparse JSON and explicit default fields compare by meaning.

For executable items, an interpretation match means equal canonical IR.
For clarification/rejection items, it means equal decision; it does **not**
prove that the explanation or offered alternatives are sufficient. Dialogue
acts are counted separately. Per-record output retains both predicted and
expected execution results. An execution error or differential mismatch is
retained as an error, never converted into a successful answer.

Aggregate-backed current queries are compared with the existing engine oracle.
Supported historical shapes are compared with raw historical marginals. Lists,
lookups, engine-only queries and static answers are labelled separately; a
successful single backend is not an independent correctness proof.

## Limits

This round was used while implementing the interpreter. A high score here is
training/development evidence, **not** a held-out estimate of Portuguese
comprehension or proof of product-market fit. Category weights are artificial.
Finite examples cannot exhaust Portuguese or every combination of filters.

The older frozen suites are also run and reported without rewriting their
labels. Some older labels assume unavailable historical stock even when the
24 archives supplied to the runner make it available; keep these disagreements
visible. Uncovered syntax must clarify or reject rather than silently dropping
a condition. H-blind remains untouched and unexecuted.

See `docs/intelligence/results/commercial-round-01/REPORT.md` and `RUNBOOK.md`
for the measured results, commands, raw evidence and the next-round procedure.
