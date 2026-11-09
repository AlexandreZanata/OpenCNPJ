# Skills Export 01 — dataset card

Questions that ask for the answer as a file (Excel, CSV or TXT), optionally with a list of
columns, plus dialogues that request or refine a file in follow-up turns. Design:
`docs/intelligence/16-SKILLS.md`.

| Property | Value |
|---|---|
| Items | see `manifest.json` (`items`, `families`, `sha256`) |
| Generator | `author_skills_export.py` (deterministic, no randomness) |
| Data anchor | 2026-10-11, competence 2026-10, catalogues municipalities@202610, sectors@1 |
| Labels | Gold IR **and** gold export (`gold.export`: format, fields, rows) built from templates, never from the interpreter |
| Human validation | Not performed (`meta.human_validated = false`) |

## Families

| Family | Surface |
|---|---|
| `format_prefix/*` | "Me mande um Excel com as padarias que fecharam em 2025 em Cuiabá/MT" |
| `format_suffix/*` | "Liste os restaurantes abertos em 2024 em Sinop/MT em CSV" |
| `with_fields/*` | "Gere um CSV com nome, CNPJ e telefone das farmácias ativas de Rondonópolis/MT" |
| `only_fields/*` | "Me mande apenas o nome e o número em um Excel com todas as empresas que tiveram baixa em Mato Grosso no setor de TI" |
| `analytical/rank` | "Exporte em Excel o ranking das 10 cidades de MT com mais padarias ativas" |
| `dialogue/*` | Count → "Agora me mande isso em Excel" → "Só o nome e o CNPJ" → "Agora em CSV"; count → "E só as matrizes?" → "Agora liste esses estabelecimentos" → "Me mande isso em TXT" |
| `stop/*` | A file without a subject (clarify); revenue, profit, partners or owners in a file (reject) |

Data parts: active stock, closures in a year ("fecharam em", "baixadas/baixados em"),
registrations in a year ("abertas/abertos em"), and every closure of a sector in a state
("todas as empresas que tiveram baixa em … no setor de …"), over six sectors and eight
municipalities in MT, GO, MS and PR.

## Semantics encoded in the labels

* A file of companies is a record list (`op: list`, 20 rows shown in the chat); the file
  holds every match up to the server cap.
* Closures filter `status = closed` and, with a period, `status_date`; registrations
  filter `opened` and keep every status. Without a period every closed company is listed.
* "número" alone is the CNPJ number (the answer says so); a phone is asked as "telefone".
* Rankings and other analytical answers keep their operation and export their table.

## Regeneration

```sh
python3 crates/opencnpj-lab/eval/skills-export-01/author_skills_export.py
target/release/opencnpj-intel eval-questions --interpret-only \
  --set crates/opencnpj-lab/eval/skills-export-01/questions.jsonl --out /tmp/skills-export-01
```

The unit test `skills_export_round_is_interpreted_exactly` replays the set on every
`cargo test`.
