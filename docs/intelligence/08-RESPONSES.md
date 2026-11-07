# 08 — Natural answers without text generation

Answers are assembled from templates and typed blocks. Every value is interpolated from
the executor's result or provenance (SR-11); templates contain no numbers of their own.

## 1. Answer envelope

```json
{
  "decision": "execute",
  "understood": "Estabelecimentos ativos · Padaria (4721-1/01, 4721-1/02) · Sorriso/MT · RFB 2026-10",
  "blocks": [
    { "type": "text", "text": "Segundo a base da Receita Federal de outubro de 2026, Sorriso (MT) tem 187 estabelecimentos ativos de padaria." },
    { "type": "kpi", "label": "Estabelecimentos ativos", "value": 187 },
    { "type": "methodology", "items": ["Atividade principal (CNAE) 4721-1/01 e 4721-1/02", "Situação cadastral: ativa", "Unidade: estabelecimento (CNPJ de 14 dígitos)"] },
    { "type": "sources", "items": [{ "name": "Receita Federal — Dados Abertos do CNPJ", "competence": "2026-10", "snapshot_sha256": "76f36078…" }] }
  ],
  "suggestions": ["Comparar com Sinop", "Ver por porte", "Listar os estabelecimentos"],
  "query": { "...canonical IR as JSON..." },
  "context": "<opaque token, 06-CONTEXT.md>",
  "timing_us": { "normalize": 4, "parse": 21, "validate": 3, "execute": 310, "render": 12 }
}
```

(The number 187 above is illustrative, not a measurement.)

Block types: `text`, `kpi`, `comparison` (members, values, absolute and relative
difference), `table` (columns, rows, total, cursor), `chart` (`bar` | `line`, series data;
always paired with a `table` for accessibility), `records` (B2B list rows, allowed fields
only), `company` (lookup card), `methodology`, `sources`, `caveats`, `clarify` (question +
options, each option carrying its IR), `notice` (defaults applied).

## 2. Templates

Stored as data (`crates/opencnpj-insights/templates/pt-BR/*.toml`), keyed by
`(op, metric kind, result shape, turn kind)`:

```toml
[count.stock.scalar.first]
variants = [
  "Segundo a base da Receita Federal de {competence:month_year}, {place} tem {value:int} {unit:plural(value)} {status:adj(value)} de {sector:label}.",
  "{place} tem {value:int} {unit:plural(value)} {status:adj(value)} de {sector:label}, de acordo com o cadastro da Receita Federal de {competence:month_year}.",
]

[count.stock.scalar.followup]       # "E em Sinop?" → short form
variants = ["Em {place}: {value:int} {unit:plural(value)}."]

[compare.per10k.two]
variants = [
  "{a.place} tem {a.value:dec1} {sector:label_plural} ativas por 10 mil habitantes, contra {b.value:dec1} em {b.place} ({diff_pct:pct1} {diff_dir})."
]
```

Formatter functions (small, in-house, no template engine dependency): `int` (pt-BR
thousands `1.234.567`), `dec1` (`12,3`), `pct1` (`12,3%`), `month_year` ("outubro de 2026"),
`date` ("11/10/2026"), `plural(n)` / `adj(n)` (agreement: "1 estabelecimento ativo",
"2 estabelecimentos ativos"), `label` / `label_plural` (catalogue display names), `list`
("Sorriso, Sinop e Lucas do Rio Verde"), `place` (display name + UF, accented IBGE name).

### Deterministic variation

The variant is chosen by `xxh3(canonical IR) mod variants.len()`: the same question
always gets the same wording (stable tests, cache-friendly), different questions read
less repetitive. Follow-up turns use the `followup` forms. Nothing is sampled at random.

### Lints (tests over the template files)

* No digit characters outside `{…}` placeholders (SR-11).
* Every placeholder exists in the result/provenance schema of its key.
* Every `(op, metric, shape)` the planner can produce has a template; every template key
  is reachable.

## 3. What each intent returns

| Intent | Blocks |
|---|---|
| Count | text + kpi + methodology + sources (+ notice for defaults) |
| Compare | text + comparison (+ bar chart + table) + methodology (population source when per capita) |
| Rank | text (top 3 in words) + table (k rows, value, base) + bar chart + note on entries removed by the minimum base |
| Breakdown | text + table + bar chart |
| Trend | text (first → last, change) + line chart + table |
| List | text ("Encontrei N estabelecimentos…") + records (page) + cursor + export hint (if the plan allows) |
| Lookup | company card (same fields and privacy as `/api/v1/cnpj`) |
| Meta | text + sources (competence, snapshot SHA-256, catalogue versions, IBGE tables) |
| Clarify | clarify block |
| Reject | text + suggestions |

## 4. Caveat catalogue

Caveats are attached by IR features (never by keywords in the answer) and rendered in
the `caveats` block:

| Id | Trigger | Text (pt-BR, abbreviated) |
|---|---|---|
| C-REG | metric uses `data_inicio_atividade` | "Data de início de atividade é a data cadastral; não comprova a abertura física do estabelecimento." |
| C-CLOSE | `Closures` | "Baixas são contadas pela data da situação cadastral atual (a última alteração)." |
| C-DENS | `Per10k` / `InhabitantsPer` | "Densidade por habitante não indica, sozinha, oportunidade de mercado." + population source and year |
| C-CAP | `capital` filter or ordering | "Capital social declarado não equivale a faturamento." |
| C-UNIT | unit inferred from "empresas" | "Contei estabelecimentos (cada CNPJ de 14 dígitos). Para contar só sedes, peça ‘só matrizes’." |
| C-SECT | sector from the catalogue | lists the CNAE codes; "atividade principal" or "principal ou secundária" |
| C-PARTIAL | period ends at the data anchor | "O período vai até {anchor:date}, data mais recente da base." |
| C-MINBASE | ranking with min base | "Locais com menos de {min_base:int} estabelecimentos não entram na variação percentual." |
| C-MEI | MEI filters | "MEI é uma opção tributária registrada na tabela do Simples; o porte cadastral costuma ser ‘micro’." |

## 5. Examples (illustrative wording; numbers are placeholders)

* "Onde existe menor concentração de farmácias por habitante em Mato Grosso?" →
  "Em Mato Grosso, as menores concentrações de farmácias ativas por 10 mil habitantes estão
  em {r1.place} ({r1.value:dec1}), {r2.place} ({r2.value:dec1}) e {r3.place}
  ({r3.value:dec1})." + table of 10 + C-DENS (IBGE estimate {year}) + C-SECT (4771-7/01,
  4771-7/02, 4771-7/03).
* "Quais setores ganharam mais estabelecimentos ativos nos últimos seis meses?" →
  "Entre {from:month_year} e {to:month_year}, os setores com maior aumento de
  estabelecimentos ativos no Brasil foram …" + table (sector, change, base) + note
  "variação do estoque de ativos entre duas bases mensais".
* "Quais empresas ativas de transporte existem em Sorriso?" → "Encontrei {total:int}
  estabelecimentos ativos de transporte (CNAE 49 a 53) em Sorriso (MT). Mostrando os 20
  primeiros." + records + suggestions ("só matrizes", "com telefone", "mais recentes").
