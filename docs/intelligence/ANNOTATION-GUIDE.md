# H-dev annotation guide

Label the request a reader can infer from the words and the supplied dialogue
state. Use the [semantic rules](01-REQUIREMENTS.md#5-semantic-rules-never-broken),
[query IR](04-QUERY-IR.md), [dialogue acts](06-CONTEXT.md#2-dialogue-acts) and
[decision policy](07-CONFIDENCE-AND-SAFETY.md#3-policy). Record the expected
interpretation before running an interpreter. Keep the original pt-BR text,
including spelling, punctuation and register.

## Decision and act

- **Execute** when the operation and all required slots resolve to a supported
  request. Record the complete `gold.ir` and its `gold.intent`.
- **Execute with note** when a semantic default, partial period or supported
  fallback needs disclosure. Record the resolved IR and the disclosure in
  `gold.reason`. An obvious default for presentation, such as the first page,
  does not need a note.
- **Clarify** when a missing slot, ambiguous term, unresolved reference or
  unattached negation could change the result. Record the ambiguity in
  `gold.reason`. Use `gold.options` for up to four fully specified requests.
  A free-value question, such as asking for a capital threshold or a UF, can
  have zero options. Include the intended operation when it is identifiable.
- **Reject** when the data or policy cannot answer the request. Record why
  and the nearest supported alternative when available. Use the intended
  operation if one is identifiable. For small talk, this label means a fixed
  conversational response with no data query. The act distinguishes it from
  an out-of-domain request.

Label `gold.act` separately. Resolve an answer to the pending clarification
first. Then check for a correction, a reference to shown results, a comparison
from focus, or an elliptical addition, removal or replacement. For an
elliptical metric change, record `replace_slot` and retain the scope. For a
self-contained new question, rebuild the request using the pinned competence.
For a metadata question about a previous answer, link that turn. An earlier
example, a focus list and a result list are distinct inputs.

## Defaults and rules

- **Units and status.** `establishments` is the default for “empresas” (SR-06).
  “Empresas sediadas” counts `head_offices`. An explicit branch constraint uses
  `head_office: false`. Use active (`02`) by default for stock and lists. Use
  all cadastral statuses for registrations unless the question filters them.
  For closures, require baixa (`08`) and date the event by the status date
  (SR-01 through SR-04). A stock count filtered to `08` with no period is a
  count of currently baixadas. Stock at an earlier date needs that snapshot.
- **Geography and sectors.** Use the national scope when place is absent
  (D-08). Resolve homonyms with explicit UF or applicable dialogue focus,
  otherwise ask. List all members for a comparison and preserve union only
  when requested. Sectors name versioned sets and use main CNAE by default.
  For explicit secondary-inclusive queries, use `scope: "any"` on the CNAE
  filter. Keep the sector mapping and population reference available for
  methodology answers (SR-07, SR-11, SR-12).
- **Porte and capital.** Use `00`, `01`, `03` or `05`. Keep MEI and Simples
  as separate flags. Clarify “grande”. Reject an explicit MEI and EPP/Demais
  contradiction. Record capital in cents and treat it as company-level data
  (SR-08, SR-09). For inclusive integer ranges, convert a strict lower bound
  to the next cent or inhabitant and a strict upper bound to the prior one.
- **Time.** Resolve all periods against `2026-10-11`, the reference date in
  the H-dev fixture for competence `2026-10`. Days and months are inclusive.
  With no period for registrations, use year-to-date. For “recentemente” use
  the last 90 calendar days through the anchor. For trends with no period,
  use the last 12 months. Disclose a partial period. “Crescendo” defaults to
  active-stock change across snapshots. Where history is unavailable, label
  the registrations fallback and name it. Percentage change needs a stated
  minimum initial base, defaulting to 30 (SR-05, SR-10).
- **Population and caveats.** Use a geographic scope or dimension for density,
  with the requested source and year. This fixture defaults to the IBGE
  2025 estimate when the user leaves the population source open. Density
  rankings use a minimum base of 30 unless specified. Explain descriptive
  results using their actual numerator and denominator. Keep financial and
  opportunity claims outside the IR (SR-09, SR-11).
- **Negation and privacy.** Attach negation to the affected scope or flag,
  or clarify. Reject requests centred on a person, CPF or partners. Ordinary
  establishment lists and lookups retain deployment privacy and plan limits.
  Never put personal identifiers, private contact values or hypothetical
  answer numbers in the labels (SR-13, SR-14).

## Review format

Use one JSON object per line. Each item has a unique `id`, `text`, `origin`
and `gold`. Use `"origin":"ai-draft"` until a human reviews that item. Mark
reviewed AI drafts `human-reviewed` and human-authored items `human`. Add
the relevant `rules` tags. `context.previous` refers to an earlier item's ID.
Add `context.focus` for an implicit comparison and `context.result` for a
reference to rows shown in an earlier answer. Those rows are stipulated
fixture data. They do not report the output of a national query.

The fixture assumes a caller page size of at most 100, with a default of 20.
The `current` profile has the pinned snapshot and both fixture population
references. `history` also provides the monthly snapshots April through
October 2026. `geography` adds neighbour and distance assets. These are
annotation capabilities for the exercise. Deployment capabilities come from
the loaded assets.

Use the shape of the typed request in [04](04-QUERY-IR.md#2-rust-types-normative-sketch).
For review, use TOM municipality codes (Sinop `8985`, São Paulo `7107`, Sorriso
`9907`), UF abbreviations and versioned sector keys instead of dense IDs.
The fixture uses `bakery@v1` for `4721101` and `4721102`, `pharmacy@v1` for
`4771701`, `4771702` and `4771703` (human pharmacies, including homeopathic),
and `transport@v1` for divisions `49` through `53`, following the audit's
transport scope. WP-1.3 will establish the curated catalogue. Since the
curated catalogue (review 2026-11-07), “transportadora(s)” means road freight
carriers, `truck_freight@v1` (4930-2/01, 4930-2/02); `transport@v1` keeps the
generic “transporte” (divisions 49–51). WP-2.1 will
map these references to canonical typed IR. In review JSON, use ISO dates
and competence strings, stable operation tags, sorted unique sets and an
explicit status list. Omit unspecified filter fields. Store one-based pages.

```json
{
  "id": "h-dev-0001",
  "text": "Quantas padarias existem em Sorriso?",
  "origin": "ai-draft",
  "rules": ["SR-06", "SR-07", "SR-11", "SR-12"],
  "gold": {
    "act": "new_query",
    "intent": "count",
    "decision": "execute_with_note",
    "reason": "Default to establishments with active cadastral status.",
    "ir": {
      "op": {"kind": "count", "metric": "stock"},
      "unit": "establishments",
      "scope": {"status": ["02"], "place": {"municipality": ["9907"]}, "activity": {"sector": ["bakery@v1"]}},
      "time": {"competence": "2026-10", "anchor": "2026-10-11"},
      "output": {"page": 1, "with_methodology": false}
    }
  }
}
```

Keep the JSONL under `/data/opencnpj-data/intelligence/eval/h-dev.jsonl`.
The acceptance test reads it locally. `OPENCNPJ_H_DEV_PATH` can select a
different local copy. The full-artifact test is explicit because a clean
checkout does not contain the dataset.

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo test -p opencnpj-lab --test h_dev h_dev_matches_annotation_ir -- --ignored --exact --nocapture
```

For review, check the intended decision first, then verify the operation,
units, geographic and activity references, modifiers, time and dialogue act.
Record the sample hash and any item IDs whose text or label needs correction.
After resolving discrepancies, record which items the owner reviewed. Keep
other AI drafts flagged. This development set is for tuning and error analysis.

## Worked examples

All examples use the fixture above. Execute and note where a default or
fallback is named. An explicit status overrides the stock default.

### Counts and filters

1. **“Quantas padarias existem em Sorriso?”** Execute with note. Count
   active establishments in `9907`, sector `bakery@v1`.
2. **“qtas padarias tem em sorrisso mt?”** Execute with the same note and IR
   as example 1. The UF supports the municipality resolution.
3. **“Quantas empresas sediadas na Bahia estão ativas?”** Execute. Count
   head offices in BA with status `02`.
4. **“Quero o número de filiais ativas em São Paulo capital.”** Execute.
   Count establishments in `7107`, status `02`, `head_office: false`.
5. **“Quantas empresas tem?”** Execute with note. National active stock.
6. **“Quantas ME sem MEI estão ativas em MT?”** Execute. MT, status `02`,
   porte `01`, `mei: false`.
7. **“Quantas empresas grandes há em Sorriso?”** Clarify. Offer porte `05`
   in Sorriso and ask for a threshold if the user means capital.
8. **“Quantas empresas são MEI e ao mesmo tempo porte Demais no Paraná?”**
   Reject. These filters conflict.
9. **“Em MT, conte as farmácias ativas tirando Sinop da conta.”** Execute.
   MT, `pharmacy@v1`, status `02`, exclude TOM `8985`.
10. **“Qual o total em MT de padarias e farmácias somadas?”** Execute with
    note. Stock in MT, using the union of the two sector sets.
11. **“Quero o total de 4721102 como CNAE principal ou secundário em MT.”**
    Execute with note. Active stock with explicit code `4721102` and `any`
    activity scope.
12. **“Quantos estabelecimentos ativos em SC declaram até 50 mil reais de capital?”**
    Execute. SC, status `02`, maximum 5,000,000 cents. Explain company-level capital.
13. **“Quantas padarias tem em Bom Jesus?”** Clarify. Ask which UF the user means.
14. **“Quero um número de empresas de tecnologia em MT.”** Clarify the sector
    boundary, for example IT services or electronics manufacturing.

### Time, density and rankings

15. **“Quantos registros com início de atividade em 2025 em Mato Grosso?”**
    Execute. Registrations for `2025-01-01` through `2025-12-31`, any status.
16. **“Quantas padarias estavam ativas em 2025 em Sorriso?”** Reject under
    `current`. Offer registrations in 2025 as a different metric.
17. **“Quantas empresas abriram neste ano em Rondônia?”** Execute with note.
    Registrations from `2026-01-01` through `2026-10-11`.
18. **“Quantas farmácias foram registradas na Bahia recentemente?”** Execute
    with note. Registrations from `2026-07-14` through `2026-10-11`.
19. **“Quantas baixas cadastrais ocorreram no ES ano passado?”** Execute.
    Closures with status `08` and status date in 2025.
20. **“No Pará, quantos CNPJs estão na situação inapta?”** Execute. Current
    stock with status `04` and no event period.
21. **“Quantas padarias não fecharam em Sinop?”** Clarify whether the user
    wants active stock or registrations filtered to exclude baixadas.
22. **“Quais cidades de MT mais ganharam estabelecimentos ativos nos últimos seis meses?”**
    Under `current`, execute with a note naming the registrations fallback
    and its `2026-04-12` through `2026-10-11` window.
23. **“Quais municípios de Goiás mais ganharam ativos entre abril e outubro de 2026?”**
    Under `history`, rank municipalities by absolute active-stock difference
    from `2026-04` to `2026-10`.
24. **“Onde tem menos farmácias por 10 mil habitantes em MT? Quero as dez primeiras, IBGE 2025.”**
    Execute with note. Ascending municipal `per10k` rank, `k=10`, minimum
    base 30, `pharmacy@v1`, IBGE estimate 2025.
25. **“Quantos habitantes por padaria ativa em Sorriso com a estimativa IBGE 2025?”**
    Execute. `inhabitants_per` divides population by active bakery stock.
26. **“Quais cidades do ES têm menos farmácias ativas, só cidades abaixo de cinquenta mil habitantes?”**
    Execute. Ascending stock rank with population bounded at 49,999.
27. **“Ativos em cidades vizinhas de Sinop, quantos são na base sem dados de geografia?”**
    Reject under `current`. The same intent under `geography` uses the
    neighbours of `8985` and status `02`.

### Lists, comparisons, breakdowns and lookups

28. **“Em Sorriso, lista as padarias ativas da página 2 com 10 por página.”**
    Execute. List active bakeries in `9907`, page 2, limit 10, CNPJ order.
29. **“Lista as transportadoras de MT com telefone e e-mail.”** Execute with
    note. List active `truck_freight@v1` in MT, contact `both`. Enforce privacy
    switches when producing rows.
30. **“Em Sorriso, quanto dá padarias e farmácias, uma contagem pra cada?”**
    Execute with note. Compare the two sectors on stock in `9907`.
31. **“Entre RJ, MG e SP, quantas filiais ativas cada estado tem?”** Execute.
    Compare `[MG,RJ,SP]` with status `02` and `head_office: false`.
32. **“Distribua em percentuais por porte o total ativo de MG.”** Execute.
    Break down `share` by `porte` with MG active stock as denominator.
33. **“Mostre a evolução mensal dos registros no Pará em 2025.”** Execute.
    Trend, registrations, month grain, whole 2025, PA.
34. **“Consulta o CNPJ 00.000.000/0001-91.”** Execute. Lookup
    `00000000000191` after validating the check digits.
35. **“Busca a empresa pela raiz 12abc345.”** Execute. Lookup root `12ABC345`.
36. **“Consulta o CNPJ 33.000.167/0001-02.”** Reject because check digits
    fail. Ask for a corrected CNPJ.
37. **“Liste os sócios das farmácias de Sorriso.”** Reject under privacy policy.

### Dialogue and scope

38. **“E em Sinop?”** After example 1, replace the place with `8985` and
    keep the bakery filter and active status.
39. **“Agora considera apenas as matrizes.”** After example 1, add
    `head_office: true` to the prior count.
40. **“Pode tirar o filtro de MEI e manter só microempresas?”** After example
    6, remove the `mei` restriction and keep porte `01`, MT and status `02`.
41. **“Demais, o porte da Receita.”** After example 7, answer the
    clarification with porte `05`, retaining place `9907` and active stock.
42. **“Mostra as farmácias daquelas cidades.”** After example 24 with Sinop
    and Sorriso as stipulated result rows, list active pharmacies in those
    two municipalities.
43. **“Quantas filiais suspensas há no Rio de Janeiro?”** After example 1,
    start a new query, counting RJ branches with status `03`. Drop the bakery
    and municipality filters.
44. **“Quais CNAEs entraram nessa conta das padarias?”** After example 1,
    execute metadata topic `cnaes` with example 1 as context.
45. **“Quanto foi o faturamento das farmácias de Sorriso?”** Reject. The
    snapshot has no revenue field. Offer a count of establishments.
