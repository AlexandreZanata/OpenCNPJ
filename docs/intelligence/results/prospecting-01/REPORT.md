# Prospecting 01 — evaluation tasks and live check (2026-11-10)

Change: prospecting questions, closed vocabulary and the assistant in the product server
([17-PROSPECTING.md](../../17-PROSPECTING.md)). Baseline: `main` at cd9b804. Data: RFB
2026-10 (`2026-10-balanced-1.2.ocnpj` and its aggregates). Release builds, on the host of
[skills-export-01](../skills-export-01/REPORT.md).

## Delivery gate

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Formatting and clippy are clean; every test binary reports `ok` (0 failed), including the
replay of this set (`prospecting_round_is_interpreted_exactly`) and the server contract
`platform_assistant_routes_and_web_front_end`.

## Evaluation tasks

```sh
scripts/eval/run_tasks.sh <opencnpj-intel from main cd9b804> target/release/opencnpj-intel \
  docs/intelligence/results/prospecting-01 /data/opencnpj-data
```

Wall time 15 min 42 s.

### ET-02 / ET-06 — interpretation regression against `main`

Raw output: [regression.txt](regression.txt); every changed item in [changes.jsonl](changes.jsonl).

```
adv	items=304	before_correct=187	after_correct=188	changed=2
commercial-round-01	items=1000	before_correct=950	after_correct=950	changed=0
commercial-round-02	items=1000	before_correct=900	after_correct=900	changed=0
commercial-round-03	items=3000	before_correct=2694	after_correct=2694	changed=0
commercial-round-04	items=5000	before_correct=4576	after_correct=4576	changed=0
commercial-round-05	items=10000	before_correct=9154	after_correct=9154	changed=0
commercial-round-06	items=20000	before_correct=18312	after_correct=18312	changed=0
commercial-round-07	items=20000	before_correct=18312	after_correct=18312	changed=0
commercial-round-08	items=50000	before_correct=45759	after_correct=45759	changed=0
commercial-round-09	items=50000	before_correct=47975	after_correct=47975	changed=0
commercial-round-10	items=50000	before_correct=48000	after_correct=48000	changed=0
commercial-round-11	items=50000	before_correct=48000	after_correct=48000	changed=0
commercial-round-12	items=50000	before_correct=47500	after_correct=47500	changed=0
commercial-round-13	items=50000	before_correct=47800	after_correct=47800	changed=0
commercial-round-14	items=50000	before_correct=47900	after_correct=47900	changed=0
commercial-round-15	items=50000	before_correct=47900	after_correct=47900	changed=0
dialogues	items=122	before_correct=84	after_correct=84	changed=0
h-informal	items=202	before_correct=118	after_correct=131	changed=14
ood	items=400	before_correct=400	after_correct=400	changed=0
probes-01	items=28	before_correct=28	after_correct=28	changed=0
prospecting-01	items=1476	before_correct=72	after_correct=1476	changed=1404
skills-company-01	items=1068	before_correct=1068	after_correct=1068	changed=0
skills-export-01	items=1881	before_correct=1881	after_correct=1881	changed=0
TOTAL	changed=1420	{'better': 1418, 'neutral': 2}
```

No item got worse. The two neutral changes were wrong before and still differ from their
labels: `adv-0296` ("farmácias em São Paulo que não sejam filiais") was refused and now
executes with "São Paulo" read as the state, where the label names the city; `inf-0155`
("empresas de mt matriz x filial") keeps its request and gains the stock note.

On `prospecting-01` the baseline executed 120 questions it should have asked about
(`out_of_policy_execution` in `baseline/prospecting-01/summary.json`): the dropped-word
defects of [EVALUATION-TASKS.md](../../EVALUATION-TASKS.md). The candidate executes none.
The set was written with this change, so 1,476 of 1,476 documents and guards the supported
behaviour; it is not an accuracy estimate for unseen questions. `h-informal`, written
earlier and independently, moves from 118 to 131 of 202; most of its remaining differences
are label conventions ("cadê as padarias" labelled as a count, "em São Paulo" as the city).

### ET-03 — interpretation contracts on every set

[contracts/summary.json](contracts/summary.json): 465,481 questions, 0 findings
(`contracts/findings.jsonl` is empty).

### ET-04 — data contracts on the snapshot

[data-contracts/summary.json](data-contracts/summary.json): 631 answers checked against
the records (every 10th new question of rounds 01–02, h-informal, skills-export-01,
skills-company-01, probes-01 and prospecting-01, 148 of them from this set), 0 findings:
counts equal the records of the equivalent list, every listed record satisfies the query,
files hold the rows they promise, sentences state the number of their tile.

## Live check of the product server

Server: `opencnpj serve --data-dir … --assistant-agg … --assistant-history … --web web/dist`
with accounts required; the caller is a user on the `pro` plan.

```sh
python3 docs/intelligence/results/prospecting-01/live_check.py http://127.0.0.1:5180 <key file>
```

Raw output ([live-check.txt](live-check.txt)); each question is asked twice and the second
timing is shown. `server ms` is `timing_us.total` of the reply, `wall ms` includes HTTP,
authentication and the quota gate on loopback:

```
decision           server ms  wall ms  question -> filters
execute                 6.66     7.78  dentistas em Cuiabá/MT com telefone -> ['com telefone']
execute_with_note      20.44    21.53  novas empresas em Sinop/MT -> ['período de 14/07/2026 a 11/10/2026']
execute                12.71    14.61  academias abertas em 2024 em Goiânia/GO -> ['período de 01/01/2024 a 31/12/2024']
execute                11.80    13.05  telefones de imobiliárias em Londrina/PR -> ['com telefone']
execute                 9.78    10.80  restaurantes em Sinop/MT com telefone e capital acima de 50 mil -> ['com telefone', 'capital acima de R$ 50.000']
execute_with_note      25.68    26.78  liste os maiores restaurantes de Goiânia/GO -> []
execute_with_note      11.42    12.36  supermercados em Sinop e Sorriso -> []
execute                19.74    21.00  Quantas farmácias ativas existem em Sinop/MT? -> []
execute                 1.92     3.16  Quais as 10 cidades de MT com mais padarias ativas? -> []
execute                 7.79     9.07  33.000.167/0001-01 -> []
execute                17.07    18.22  filiais do Bradesco em MT -> []
clarify                 0.48     1.69  empresas de energia solar em Sinop -> []
clarify                 0.13     1.18  liste as farmácias de Sinop/MT com DDD 66 -> []
clarify                 0.13     0.79  preciso prospectar clientes para minha distribuidora de bebidas em Cuiabá -> []
reject                  0.03     0.53  sócios da empresa Zaflas -> []
15 questions, 0 unexpected decisions
```

Also checked by hand on this server (2026-11-10):

* Without a credential `POST /api/v1/assistant/ask` answers `401 unauthenticated`; with a
  login key or a session token it answers and returns `X-Quota-Daily-Remaining`.
* A per-account daily limit of 8 set in the administration page stopped the ninth request
  of the day with `429 quota_exceeded`.
* A `free` account asking for a file gets `403 plan_forbids_export`; a `pro` account
  receives the workbook (`X-OpenCNPJ-Rows: 321` for the bakeries of Sinop/MT).
* The usage of the account lists the kind `assistant`, and the search log stores the
  question of each turn.
