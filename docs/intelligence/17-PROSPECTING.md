# 17 — Prospecting questions: search-style input, closed vocabulary, one product server

People who look for customers do not write analyst questions. They type what they would
type into a search box ("dentistas em Cuiabá com telefone"), ask for contacts ("telefones
de imobiliárias em Sinop") and mix filters freely ("restaurantes novos com capital acima de
50 mil"). An audit of the interpreter with such questions (2026-11-10) found two problems:

* **Refusals.** A noun phrase without a verb was "not a cadastral search" unless it was
  exactly an activity plus a city.
* **Silently wrong answers.** A question with a request verb executed even when it held a
  word the grammar did not know, and that word was dropped: "liste as empresas de energia
  solar em Sinop" listed every company of the city; "abertas hoje", "com DDD 66", "de médio
  porte", "com mais de 10 anos" and "capital acima de 100 mil" were ignored the same way;
  "Sinop e Sorriso" kept only the first city.

This document describes the changes that close both, and how the assistant became part of
the product server. Evidence: [results/prospecting-01](results/prospecting-01/REPORT.md).

## 1. Closed vocabulary

A question is executed only when every word outside a recognized place or activity is known
to the grammar (`crates/opencnpj-nlu/src/vocabulary.rs`, data in
`lang/pt-BR/vocabulary.toml`). An unknown word names something no filter can apply, so the
answer is a question about it, never a broader result:

> Não reconheci “energia”, “solar”. Diga a atividade ou o filtro com outras palavras, ou
> informe o código CNAE.

The vocabulary has two tables:

| Table | Holds | Used by |
|---|---|---|
| `[search]` | Function words, company nouns, status, size, regime, legal form, amounts, contacts and time words: every one is read by a filter, a period or the sentence structure | Search-style questions (section 2) |
| `[general]` | Request verbs, question words, analysis terms, scenario phrases and regional expressions; `corpus` lists the remaining words of executed evaluation questions that carry no filter meaning | Questions that state an intent ("quantas", "liste", "compare") |

Rules around the check:

* Numbers, codes and single letters are not vocabulary: a number must be read by a rule (a
  date, a row count, an amount, a population, a code). One that is not gets "Não consegui
  aplicar o número “66” a um filtro".
* A preamble closed by a comma or colon before the question word ("Para vender meu
  software, quantas…") sets the scene and is not checked.
* An explicit CNAE code pins the activity; words after it describe the code.
* The check runs last, so every earlier refusal and clarification is unchanged. Its message
  starts with "Não reconheci", which still lets a company-name search answer ("busque a
  empresa Zaflas").

`opencnpj-intel vocab --set … --out words.tsv` lists every word of the evaluation sets
outside catalogue entities with how many executed gold questions use it and whether it is
known, so the vocabulary is reviewed against the corpus instead of guessed.

## 2. Search-style questions

A phrase **headed by what is searched for** — an activity or a plural company noun, after
articles and a plain request ("quero", "procuro", "concorrentes de") — is a complete
question when every other word belongs to the `[search]` table:

| Question | Reading |
|---|---|
| `dentistas ativos em Cuiabá/MT` | Stock of active establishments (count), with the usual sample of records |
| `academias abertas em 2024 em Goiânia/GO` | Registrations in the period |
| `novas empresas em Sinop/MT`, `restaurantes novos` | Registrations of the last 90 days of the data, said in a note |
| `dentistas em Cuiabá/MT com telefone` | **List** with the contact filter (a count cannot filter by contact) |
| `empresas de energia solar em Sinop` | Clarification about the unknown words |
| `Padaria Pão Quente`, `Gráfica Novo Tempo` | A sector followed by other words may be a company name: left to the company search |

It starts a new query when it names a place or when there is no conversation to refine;
inside a conversation, "e só as matrizes?", "em Sinop" and similar remain follow-ups.

## 3. What else is read now

| Surface | Reading |
|---|---|
| `telefones de …`, `emails de …`, `contatos das …`, `leads de …`, `quem vende …` | List of the companies; phone/e-mail words set the contact filter. WhatsApp is read as "phone on record", with a note |
| `Sinop e Sorriso`, `Sinop, Sorriso e Lucas do Rio Verde` | Every coordinated place stays in the query |
| `capital acima de 100 mil`, `capital de pelo menos 50 mil`, `capital até …`, `capital entre … e …` | Capital bounds without the word "social" |
| `com mais de 10 anos`, `com menos de 2 anos` | Start of activity counted back from the data date, with a note |
| `médio porte`, `grande porte` | RFB size class "demais", with a note (the registry has no medium/large split) |
| `cooperativas`, `sociedades anônimas`, `S/A`, `EIRELI`, `associações`, `fundações` | Legal nature codes; these nouns also count as companies |
| `hoje`, `ontem`, `esta semana`, `semana passada`, `este mês`, `mês passado`, `últimas N semanas` | Periods relative to the **data date**, said in a note |
| `os maiores restaurantes de Cuiabá` | List ordered by declared capital, the only size the registry holds, with a note |
| `fechou`, `encerrou`, `faliu`; `sem ser MEI` | Singular event verbs and one more MEI negation |
| `supemercados`, `acadmias`; `em curitba` | A misspelt activity or closing city is recovered when exactly one catalogue name is one edit away; the answer says so |
| `bairro`, `CEP`, `DDD`, `rua`, `avenida` | Not interpreted in the conversation: a clarification (the search page filters by CEP prefix and area code through the API) |
| `preciso prospectar clientes para minha distribuidora` | The activity after "para minha/meu" is the asker's own business: the assistant asks what kind of company to find |
| `filiais do Bradesco em MT`, `unidades da Havan em Goiás` | Company-name search without the establishment noun; a name that is both a state and a city reads as the state |

Catalogue: twelve sectors were appended (driving schools, medical practices, beverage
wholesale, internet providers, car dealers, higher education, notaries, lottery shops,
funeral homes and the broad `wholesale`, `retail` and `manufacturing`, which hold every
subclass of CNAE division 46, division 47 and section C) and common names were added to 28
existing sectors ("construtora", "clínica odontológica", "posto de combustível", "empresa
de software"…). Sector ids only grew at the end.

## 4. Answers

* The answer carries `filters`: the filters applied besides place and activity ("com
  telefone", "MEI", "capital acima de R$ 50.000", "abertas até 11/10/2016"). What was applied
  is shown, never implied.
* A list filtered by contact shows the phone and/or e-mail column.
* A period that is one day, starts inside a month or lies within one month is stated by its
  exact days ("De 05/10/2026 a 11/10/2026, …") instead of month names.

## 5. One product server

The assistant, its assets and the engine executor moved from the evaluation crate into the
product crate (`crates/opencnpj/src/assistant/`); `opencnpj-lab` keeps the evaluation,
differential and contract tools and re-exports what it needs.

`opencnpj serve --assistant-agg FILE` loads the assistant over the served snapshot and adds
two **data routes**, so every turn and every file is authenticated, checked against the
caller's plan and counted in the caller's quota:

| Route | Body | Result |
|---|---|---|
| `POST /api/v1/assistant/ask` | `{"question", "context"?}` | The reply of `16-SKILLS.md` §5 plus `answer.filters`. Usage kind `assistant`; the search log stores the question |
| `POST /api/v1/assistant/export` | `{"request", "export", "question"?}` | The file. Usage kind `export`; needs a plan with export; rows are capped by the plan's `max_export_rows` and `--assistant-export-rows` |

`--web DIR` serves the built front end (`web/dist`) at `/` on both listeners. See
`docs/API.md` and `web/README.md`.

## 6. Evaluation

`crates/opencnpj-lab/eval/prospecting-01` holds 1,476 questions in 47 families with gold
requests, company searches and clarifications built from templates
(`author_prospecting.py`), replayed by `cargo test` and by `scripts/eval/run_tasks.sh`.
The set was written with these changes, so its score documents the supported behaviour and
guards it; it is not a measure of generalization. `h-informal`, written earlier and
independently, is the closer proxy for unseen phrasing.

## 7. Open points

* The vocabulary is closed on purpose; real usage will show words to add. The search log
  (`/admin/api/searches`) holds every question that got "Não reconheci": review it monthly
  (ET-10) and add words or sectors with a probe.
* A known word in a construction no rule reads can still be dropped ("empresas que mais
  crescem"). The number check covers digits; other constructions need probes as they appear.
* Activities outside the sector catalogue get a clarification. Matching the unknown words
  against the official CNAE descriptions and offering the candidates as options is the next
  step for long-tail activities.
* Neighbourhood, CEP and area-code filters are not part of the query IR.
* The assistant binds to the snapshot served at start; switching snapshots needs a restart
  with the matching aggregates.
