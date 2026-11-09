# Skills Export 01 — results (2026-11-09)

Design: [16-SKILLS.md](../../16-SKILLS.md). Dataset: `crates/opencnpj-lab/eval/skills-export-01`
(`questions.jsonl` sha256 `bfbfe7e2cedd36209e47b4cf2956647a08d8f415e9678f84433fc1522480f7b2`).

Host: 13th Gen Intel Core i7-13620H, 16 logical CPUs, 31 GiB RAM, release build
(`cargo build --release -p opencnpj-lab --bin opencnpj-intel`). Data: RFB 2026-10,
snapshot `2026-10-balanced-1.2.ocnpj` (sha256 `76f36078…6037b`), aggregate `2026-10.ocagg`.

## 1. Interpretation of the skills set

```sh
target/release/opencnpj-intel eval-questions --interpret-only \
  --set crates/opencnpj-lab/eval/skills-export-01/questions.jsonl --out $OUT
```

Raw counts (`summary.json`):

```json
{"act_correct": 1614, "act_items": 1614, "decision_correct": 1674, "interpretation_correct": 1674, "items": 1674}
```

`interpretation_correct` requires the canonical request **and** the export spec (format,
columns, rows) to equal the gold. A check with two deliberately corrupted gold exports
(one format, one column list) reported `interpretation_wrong: 2`, so the export is scored.

Limits: the set is synthetic and template-built (the same author wrote the skill), so
it measures coverage of the documented surfaces, not generalization to real users.
No human validation was performed.

## 2. No regression on earlier sets

Every earlier set was interpreted with the binary before this change (`150d55e`) and
with the final binary; predictions (decision and IR) were compared item by item.

```sh
opencnpj-intel eval-questions --interpret-only --set <set> --out <dir>   # both binaries
python3 diff_preds.py   # joins predictions.jsonl by id, compares (decision, ir)
```

Raw output: [regression.txt](regression.txt) — 460,028 items over 18 sets,
`total changed 0`, identical `interpretation_correct` per set.

## 3. Correctness of a file against an independent engine

"me mande apenas o nome e o numero em um exel com todas as empresas que tiveram baixa
em mato grosso no setor de TI" produced a workbook with 1,052 rows (plus header). The
`opencnpj` search engine, which shares no code with the interpreter or the assistant,
returns the same total for the same filters:

```sh
opencnpj search $SNAPSHOT --filter uf=MT \
  --filter cnae=6201501,6201502,6202300,6203100,6204000,6209100 --filter situacao=08 --limit 1
# {'expansion_truncated': False, 'fuzzy_used': False, 'next_cursor': '66037',
#  'snapshot': {'competence': '2026-10', 'source_snapshot': 'D61010'}, 'total': 1052}
```

The workbook opens in openpyxl 3.1.5 (sheets `Dados`, `Sobre`; `A1:B1053`, frozen at
`A2`, filter `A1:B1053`) and converts with LibreOffice to a 1,053-line CSV. The count
"Quantas padarias tiveram baixa em Cuiabá/MT em 2025?" (aggregates: 185) and its
follow-up file "Agora me mande isso em CSV" (snapshot records: 185 rows) agree.

## 4. Latency of the running service

```sh
opencnpj-intel serve --agg … --history … --snapshot … --web web/dist \
  --listen 127.0.0.1:5175 --listen 172.19.2.14:5175 --allow-network 172.19.2.0/24
python3 latency.py http://127.0.0.1:5175 web/src/features/chat/examples.ts
```

Start-up (catalogues, aggregates, snapshot hash and open): `assets loaded in 22787 ms`.
Each example was asked three times; the median is reported. `wall_ms` is the HTTP round
trip, `engine_ms` the service's own `timing_us.total`, `interp_us` the interpretation.
`file` rows time `POST /api/local/export` once. Raw output: [latency.txt](latency.txt).

Observations from the raw output:

* Interpretation: 155–640 µs per question.
* Answers: 1.8–42 ms for sector-scoped questions, rankings and comparisons. Two
  questions over **all activities** ("empresas … em Sorriso/MT em 2025", "evolução
  mensal de registros de empresas em Cuiabá") take 375–435 ms: the aggregate reader
  decompresses and checksums every CNAE block of the flow table on each request. A
  decoded-block cache in `opencnpj-insights::agg` is the next performance item.
* Files: 3.6 ms (10-row ranking) to 1.7 s (1,167-row workbook, first read of those
  records); the same records as CSV right after took 272 ms.

## 5. Activity rollup (all-activity questions)

`Aggregate::activity_rollup` sums each table's cells over every CNAE (keys are
CNAE-major, so a question without an activity used to decode the whole table). Readers
use it only when no CNAE filter or exclusion applies and the grouping is not by sector
or CNAE; counts are additive, so totals are unchanged. The service builds the rollups
at start-up (`assets loaded in 13542 ms`, warm page cache).

Equality, old (`150d55e`+skills) vs new binary, aggregates only:

```sh
opencnpj-intel eval-questions --set crates/opencnpj-lab/eval/commercial-round-0{1,2}/questions.jsonl \
  --agg $AGG --history $HISTORY --out <dir>     # both binaries; compare status/value/total per item
# round 01: executions compared=1790 different=0
# round 02: executions compared=1740 different=0
```

A battery of 14 all-activity questions (closures, registrations, trends, net
registrations, density, rankings by city and UF, porte/status/CNAE breakdowns) through
`ask --agg … --history …` returned identical values on both binaries (`different=0`).

Latency after the change, same command as §4: [latency-rollup.txt](latency-rollup.txt).

| Question | Before (`engine_ms`) | After |
|---|---|---|
| Quantas empresas tiveram baixa em Sorriso/MT em 2025? | 433.4 | 19.0 |
| Mostre a evolução mensal de registros de empresas em Cuiabá/MT em 2025. | 375.2 | 3.5 |
