# opencnpj-nlu

A deterministic Brazilian Portuguese interpreter for commercial CNPJ research.
The public `Interpreter` consumes text, a catalogue-backed `LanguagePack`, the
served data's `Capabilities`, a data reference date and explicit `Conversation`
state. It returns execute, execute-with-note, clarify or reject. Only canonical
requests accepted by `opencnpj-query` validation can be executed.

The implementation includes Unicode folding, token-level abbreviation rewrites,
a longest-match catalogue trie, limited unique geographic typo recovery, sector
and place disambiguation, date/quantity parsing, typed filter resolution,
commercial query constructions, conversational state and a refusal policy.
Cue phrases and policy vocabulary are small checked-in TOML files. Municipality
and sector vocabulary comes from the existing verified catalogue, including
popular sector names and current core CNAE sets. No LLM, online service,
classifier training, external question dataset or Python runtime is required.

Supported request families are counts, lists, CNPJ/root lookup, comparisons,
rankings, breakdowns, flow series, historical active stock/change, density and
methodology. Unrecognized or unsupported syntax can clarify or reject. Grammar
coverage is finite; inspect measured suite results rather than assuming all
Portuguese constructions are supported.

The host supplies actual historical availability and population references.
Dates use the data anchor, not the clock. New subjects reset filters, follow-ups
can replace/add/remove conditions, homonym clarifications offer validated
alternatives, and result references use members supplied by the host after
execution. Local state is not a signed network transport.

Run questions with `opencnpj-intel ask` and batch audits with
`opencnpj-intel eval-questions`; see the
[runbook](../../docs/intelligence/results/commercial-round-01/RUNBOOK.md).
The 1,000-question development round and older-suite results are recorded in the
[report](../../docs/intelligence/results/commercial-round-01/REPORT.md).

The [round-08 review](../../docs/intelligence/results/commercial-round-08/REPORT.md)
audits the later development sets, including incorrect closure-date labels.
Full date literals retain their separators and cannot be mistaken for CNPJ
roots. Numeric capital/population filters are distinct from identifier lookup;
root lookups accept an explicit identifier cue or a standalone identifier.
Historical stock needs an explicit available month; a year alone does
not silently select December. Registration dates remain separate from closure
dates and are retained on filtered lists and stock counts. Missing closure
periods prompt clarification instead of reusing a registration-date filter.

The design documents remain the broader target, including compiled language
packs, calibrated confidence, transport and application integration. Rule
`evidence` is an ordinal signal, not a calibrated probability. This implementation
and a synthetic score do not constitute human product validation.
