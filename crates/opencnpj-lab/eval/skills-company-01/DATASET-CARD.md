# Skills Company 01 — dataset card

Questions that look up a company by name (corporate name or trade name), optionally
scoped to a municipality or UF, plus activity searches ("busque padarias em X") that must
remain record lists and privacy/unavailable-metric refusals. Design:
`docs/intelligence/16-SKILLS.md`.

| Property | Value |
|---|---|
| Items | see `manifest.json` (`items`, `families`, `sha256`) |
| Generator | `author_skills_company.py` (deterministic, no randomness) |
| Data anchor | 2026-10-11, competence 2026-10, catalogues municipalities@202610, sectors@1 |
| Labels | Gold company search (`gold.company`: name, municipalities, ufs) or gold IR (`gold.ir`) built from templates, never from the interpreter |
| Human validation | Not performed (`meta.human_validated = false`) |

## Families

| Family | Surface |
|---|---|
| `opener/direct` | "Encontre a empresa Zaflas", "Dados da empresa Mitel Teleinformática", "Qual o CNPJ da Padaria Pão Quente?" |
| `place/municipality` | "Qual o CNPJ da Padaria Pão Quente em Cuiabá/MT?", "Encontre a empresa Zaflas em Sorriso" |
| `place/uf` | "Informações sobre a empresa Zaflas em MT", "Ficha da empresa Mitel Teleinformática no Paraná" |
| `bare/name` | "zaflas", "Mitel Teleinformática", "Padaria Pão Quente" |
| `bare/place` | "Zaflas em Cuiabá/MT", "Mitel Teleinformática em Sorriso" |
| `regional/*` | "Bah, tchê, encontre a empresa Zaflas", "Oxe, ache a empresa Padaria Pão Quente em Sorriso, visse?" (South, Northeast, Minas, Centre-West, North, Southeast) |
| `activity_list/municipality` | "Busque padarias em Cuiabá/MT", "Encontre farmácias em Sorriso" |
| `refusal/privacy` | "Encontre o dono da empresa Zaflas", "Qual o CPF do dono da Padaria Pão Quente?" |
| `refusal/unavailable` | "Qual o faturamento da empresa Zaflas?", "Qual a receita bruta da empresa Mitel Teleinformática?" |
| `refusal/stop` | Greetings ("Bom dia") and invalid CNPJ check digits ("Consulte o CNPJ 33.000.167/0001-00") |

## Semantics encoded in the labels

* A company-name search sets `decision = execute`, `act = company_search`, `ir = null`,
  and `company = { name, municipalities, ufs }`.
* Trailing geographic qualifiers (`em Cuiabá/MT`, `em Sorriso`, `no Paraná`) are split
  into `municipalities` or `ufs` while keeping the company name intact.
* When a search verb is followed only by a sector and place (`Busque padarias em Sorriso`),
  the question lists establishments of that sector (`op: list`) rather than searching for a
  company named "padarias".
* Privacy refusals (owner, partners, CPF) and unavailable metrics (revenue, profit,
  headcount) remain refusals even when combined with a company name.

## Regeneration

```sh
python3 crates/opencnpj-lab/eval/skills-company-01/author_skills_company.py
target/release/opencnpj-intel eval-questions --interpret-only \
  --set crates/opencnpj-lab/eval/skills-company-01/questions.jsonl --out /tmp/skills-company-01
```

The unit test `skills_company_round_is_interpreted_exactly` replays the set on every
`cargo test`.
