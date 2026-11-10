# Prospecting 01 — dataset card

Questions written the way people look for customers: search-style noun phrases without a
verb, contact and lead requests, new companies, capital, age, size and legal form,
coordinated places, broad sectors, and requests that must be asked about instead of
executed. Design: `docs/intelligence/17-PROSPECTING.md`.

| Property | Value |
|---|---|
| Items | see `manifest.json` (`items`, `families`, `sha256`) |
| Generator | `author_prospecting.py` (deterministic, no randomness), gold built with the skills-export helpers |
| Data anchor | 2026-10-11, competence 2026-10, catalogues municipalities@202610, sectors@1 |
| Labels | Gold IR (`gold.ir`), gold company search (`gold.company`) or a clarification/refusal, built from templates, never from the interpreter |
| Human validation | Not performed (`meta.human_validated = false`) |

## Families

| Family | Surface | Gold |
|---|---|---|
| `search/*` | "dentistas ativos em Cuiabá/MT", "quero farmácias em Sinop/MT", "concorrentes de academias em Goiânia/GO", "MEIs ativos em Sorriso/MT", "academias abertas em 2024 em …", "novas empresas em …" | Count of the stock, or of registrations in the period |
| `contacts/*` | "telefones de …", "emails de …", "… com telefone e email", "… com whatsapp", "me dá os contatos das …", "leads de …" | List, with the contact filter the words name |
| `capital/*` | "capital acima de 100 mil", "capital de pelo menos 50 mil", "capital até 10 mil" | Capital bounds in cents |
| `age/*`, `period/*` | "com mais de 10 anos", "com menos de 2 anos", "abertas este mês", "abertas esta semana" | `opened` range counted from the data date |
| `size/*`, `legal_form/*` | "médio porte", "grande porte", "cooperativas", "sociedades anônimas" | Size class "demais"; legal nature codes |
| `order/biggest` | "liste os maiores restaurantes de …" | List ordered by declared capital |
| `places/*` | "liste as farmácias de Sinop e Sorriso", "dentistas em Cuiabá e Rondonópolis" | Both municipalities |
| `broad/*` | "quantos atacadistas existem em …", "indústrias em …" | Broad sectors (CNAE division 46, section C) |
| `prospecting/*` | "quem vende material de construção em …", "quero vender para restaurantes novos em …", "preciso prospectar clientes para minha distribuidora de bebidas em …" | List; registrations of the last 90 days; clarification when the target is not named |
| `company/*` | "filiais do Bradesco em MT", "unidades da Havan em Goiás" | Company search scoped to the state |
| `unknown/*` | "empresas de energia solar em …", "quantas empresas de criptomoedas existem em …", "… com site", "… com DDD 66", "… do bairro Centro", "… com 15 funcionarios" | Clarification (or refusal for data the registry does not hold): never executed |

## Scores

| Build | Correct | Out-of-policy executions |
|---|---:|---:|
| `main` at cd9b804 | 72 / 1,476 | 120 |
| This change | 1,476 / 1,476 | 0 |

Commands and raw output: `docs/intelligence/results/prospecting-01/REPORT.md`. The set was
written together with the change it measures: the second row documents and guards the
supported behaviour, it does not estimate accuracy on unseen questions.
