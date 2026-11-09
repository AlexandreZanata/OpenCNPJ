# Skills Company 01 and agro activities — results (2026-11-09)

Design: [16-SKILLS.md](../../16-SKILLS.md) §8–§10. Datasets:
`crates/opencnpj-lab/eval/skills-company-01` (1,068 items, sha256 `ebdecc179b6fa5eb…`) and
`crates/opencnpj-lab/eval/skills-export-01` (1,881 items, sha256 `84359bb0b38d5425…`, now with
`agro/*` and `stop/unknown_kind`). Both sets are synthetic and template-built; they measure
coverage of the documented surfaces, not generalization. No human validation.

Host and data as in [skills-export-01](../skills-export-01/REPORT.md): release build, RFB 2026-10,
snapshot `2026-10-balanced-1.2.ocnpj`, aggregate `2026-10.ocagg`.

## 1. Interpretation, against `main`

The baseline is the binary built from `origin/main` (`16395a9`) with `main`'s sector catalogue;
the new binary uses this branch's code and catalogue. Every set was interpreted by both and
compared item by item on (decision, request, company, export).

```sh
opencnpj-intel eval-questions --interpret-only [--sectors <main catalogue>] --set <set> --out <dir>
```

Raw output: [regression.txt](regression.txt). 463,977 items over 21 sets:
`TOTAL changed=1086 {'better': 1086}` — no item got worse or changed while staying wrong; every
earlier commercial, adversarial, dialogue, informal and out-of-domain item is unchanged;
skills-company-01 goes from 60 to 1,068 of 1,068 and skills-export-01 from 1,803 to 1,881 of 1,881.

Review of the previous session's draft: its `regression.txt` reported 318 changed items with
lower scores (e.g. `ood` 400 → 376). That file was stale: the code it left already scored the
baseline on `ood`, round 02 and dialogues. Two real problems were found and fixed here: bare
data questions ("empresas ativas em Várzea Grande mt", "nr de pet shop em Florianópolis") were
taken as company names, and the test name "Agropecuária (Boa) Esperança" became ambiguous with
the new agro sector and real cities (Esperança/PB, Boa Esperança/MG); it was replaced in the
generator by names that collide with no catalogue entry.

## 2. Real data

```sh
python3 live_check.py http://172.19.2.14:5175
```

Raw output: [live-check.txt](live-check.txt). Highlights:

* "Lista de empresas agricolas em sorriso mt, e um exel com todas": 952 active establishments of
  CNAE division 01 and a 952-row workbook; the page shows only agro activities. Before, the same
  question listed 23,134 establishments (banks included). Independent check:

  ```sh
  opencnpj search $SNAPSHOT --filter municipio=9907 --filter cnae=01 --filter situacao=02 --limit 1
  # independent engine total (Sorriso, CNAE division 01, ativa): 952
  ```

* "quero a lista de empresas xyzolicas de sorriso mt": a clarification, not every company.
* "encontre a empresa zaflas" / "zaflas": 4 establishments matching the name, each with a
  "Dados do CNPJ …" follow-up; "qual o CNPJ da MITEL TELEINFORMATICA?": the company card.
* "encontre o dono da zaflas": refused (privacy).
* "me mande apenas o nome e o telefone…": 1,052 rows and the note that branches of one company
  look like repeats without the CNPJ. In the user's file of that request, 75 of 1,052 lines
  repeat other lines (e.g. one company appears on 29 lines, one per closed establishment in MT). The
  same request with the CNPJ column, through `/api/local/export`: `rows 1052 distinct CNPJ 1052
  distinct (nome,telefone) 977` — no establishment is duplicated.
