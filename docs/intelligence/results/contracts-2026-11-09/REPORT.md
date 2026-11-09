# Contracts and evaluation tasks — 2026-11-09

Tasks of [EVALUATION-TASKS.md](../../EVALUATION-TASKS.md) for PR #8, run with

```sh
scripts/eval/run_tasks.sh <opencnpj-intel from main 16395a9> <candidate opencnpj-intel> \
  docs/intelligence/results/contracts-2026-11-09 /data/opencnpj-data
```

(release builds; RFB 2026-10 data; 25 min wall time on the host of
[skills-export-01](../skills-export-01/REPORT.md)).

## ET-02 / ET-06 — interpretation regression against `main`

Raw output: [regression.txt](regression.txt), every changed item in [changes.jsonl](changes.jsonl).

```
TOTAL	changed=2132	{'better': 1111, 'neutral': 1021}
```

No item got worse. The 1,021 neutral changes are meat-processing questions (1,001 directly,
20 follow-ups inheriting the sector): the catalogue no longer lists `1012105`, a CNAE that does
not exist (0 establishments in RFB 2026-10, absent from the CNAE table), so the canonical
request changes while the answer and its correctness do not. Probes 01 go from 4 to 28 of 28,
skills-company-01 from 60 to 1,068 of 1,068, skills-export-01 from 1,803 to 1,881 of 1,881.

## ET-03 — interpretation contracts on every set

[contracts/summary.json](contracts/summary.json): 464,005 questions, 0 findings
(`findings.jsonl` is empty). Calibration before the fixes found one real defect on a wrong
item (`adv-0281`, "quantos habitantes por farmácia tem Belém do Pará": Belém dropped) and
taught the exemptions documented in the contracts table.

## ET-04 — data contracts on the snapshot

[data-contracts/summary.json](data-contracts/summary.json): 483 answers checked against the
records (every 10th new question of rounds 01–02, h-informal, skills-export-01,
skills-company-01 and probes-01), 0 findings: aggregate counts equal the records of the
equivalent list, every listed record satisfies place, activity, status, head office and dates,
files hold the rows they promise, sentences state the number of their tile, exact company
searches return the searched name.

## ET-07 — live smoke after deploy (172.19.2.14:5175)

```
liste as padarias fechadas em Sorriso/MT          execute           Encontrei 140 estabelecimentos baixados de padaria em Sorriso (MT).
quantas padarias inativas existem em Sorriso/MT?  execute_with_note Sorriso (MT) tem 182 estabelecimentos nas situações suspensa, inapta e baixada de padaria…
liste as padarias de 2025 em Sorriso/MT           clarify           Esse período se refere à abertura ou à baixa das empresas?…
Lista de empresas agricolas em sorriso mt, e um…  execute           Encontrei 952 estabelecimentos ativos de atividade agropecuária em Sorriso (MT).
quantas padarias tem Belém do Pará?               execute           Belém (PA) tem 1.298 estabelecimentos ativos de padaria…
encontre a empresa zaflas                         execute           Encontrei 4 estabelecimentos para “zaflas”.
```

## Defects found and fixed in this review

| Defect | Before | After |
|---|---|---|
| "padarias fechadas / encerradas" | active bakeries | closed (status 08) |
| "padarias inativas" | active bakeries | suspended, unfit and closed, with a note |
| "sociedade anônima fechada" (after the status fix) | read as closed status: 3,458 items worse in the first regression run | legal form masked before status words; 0 worse |
| "liste as padarias de 2025", "desde 2020" | year silently dropped | clarification: opening or closure |
| "Excel com 5000 padarias" | 20 rows | 5,000 rows in the file; chat lists above 100 get the existing clamped refusal |
| "… tem Belém do Pará" | national count | Belém (PA) |
| `1012105` in meat processing | non-existent CNAE in the catalogue | removed |
| "padarias e as xyz" rule | (introduced during the review) | withdrawn: it contradicted the round-15 decision that "padarias e confeitarias" is the bakery surface |
