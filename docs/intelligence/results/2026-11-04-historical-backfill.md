# 2026-11-04 — WP-1.5 historical backfill

> **2026-11-07:** the Python scripts used below were ported to Rust and removed
> (`opencnpj-intel catalog cnae`, `opencnpj-intel agg backfill`); the ports reproduce
> their outputs byte for byte. See `2026-11-07-reviewer-tasks.md`. Commands below are kept
> as the record of what ran on 2026-11-04.

The initial November 2024 T1/T2 build found the historical UF term `BR`.
The aggregate format now assigns that value UF ID `28` and retains
the source row in T2. The corrected build reused the verified snapshot.

The D-04 window is November 2024 through October 2026. This run uses the
default Receita Federal share and the municipality catalogue from WP-1.2.
The historical archive is separate from the current T1–T5 artifact produced
by WP-1.4.

## Source preflight

**MEASURED.** Both official shares listed the same 37 ZIPs and byte sizes
for April 2025. The default share listed 37 ZIPs for each of the 24 months
in the D-04 window. This preflight measures file listings.

```bash
backfill=/data/opencnpj-data/intelligence/backfill
timeout 90s env -u RFB_BASE_URL -u RFB_SHARE_TOKEN nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj download --month 2025-04 --dry-run --dest "$backfill/raw"
timeout 90s env RFB_BASE_URL=https://arquivos.receitafederal.gov.br/public.php/webdav/Dados/Cadastros/CNPJ RFB_SHARE_TOKEN=gn672Ad4CF8N6TK nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj download --month 2025-04 --dry-run --dest "$backfill/raw"
for year in 2024 2025 2026; do
  for month in "$year"-{01..12}; do
    [[ $month < 2024-11 || $month > 2026-10 ]] && continue
    timeout 90s env -u RFB_BASE_URL -u RFB_SHARE_TOKEN nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj download --month "$month" --dry-run --dest "$backfill/raw"
  done
done
```

Raw output is in `2026-11-04-preflight.log` and
`2026-11-04-retention-preflight.log` under `/data/opencnpj-data/intelligence/backfill/`.

## Backfill

```bash
env -u RFB_BASE_URL -u RFB_SHARE_TOKEN nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj download --month 2024-11 --dest /data/opencnpj-data/intelligence/backfill/raw
env -u RFB_BASE_URL -u RFB_SHARE_TOKEN python3 -B crates/opencnpj-lab/scripts/backfill-aggregates.py
python3 -B crates/opencnpj-lab/scripts/backfill-aggregates.py --summarize
```

The runner records each expanded command, output and exit status in
`backfill/logs/<competence>/`. It samples the allocated bytes of the
monthly ZIPs, build scratch and snapshot while those commands run.
`backfill/aggregates/<competence>.json` records the snapshot SHA-256,
the source verification result, the aggregate SHA-256, the national T1/T2
counts and the sampled disk peak. A completed manifest and matching
aggregate checksum allow a restart to skip the build. The runner removes
its monthly ZIPs and snapshot after those checks.

## November 2024 acceptance [MEASURED]

```bash
backfill=/data/opencnpj-data/intelligence/backfill
snapshot="$backfill/work/2024-11/2024-11.ocnpj"
catalogue=/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv
nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj build --input "$backfill/raw/2024-11" --output "$snapshot" --competence 2024-11 --profile balanced --tmp "$backfill/work/2024-11/scratch" --workers 16 --readers 4 --report "$backfill/logs/2024-11/build.json"
nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj verify "$snapshot" --sources "$backfill/raw/2024-11" --workers 16
nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj-intel agg build --snapshot "$snapshot" --catalog "$catalogue" --out "$backfill/aggregates/2024-11.ocagg" --tables T1,T2
nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj search "$snapshot" --filter uf=BR --limit 1 | python3 -c 'import json, sys; row = json.load(sys.stdin); print("indexed_uf_br_total", row["total"])'
nice -n 19 ionice -c3 /data/opencnpj-target/release/opencnpj search "$snapshot" --filter uf=BR --filter situacao=02 --limit 1 | python3 -c 'import json, sys; row = json.load(sys.stdin); print("indexed_uf_br_active", row["total"])'
awk 'NR==1{max=$1;min=$2}{if($1>max)max=$1;if($2<min)min=$2}END{print "sampled_peak_working_bytes",max;print "sampled_minimum_volume_free_bytes",min}' "$backfill/logs/2024-11/disk.tsv"
find "$backfill/aggregates" -maxdepth 1 -type f -name '*.ocagg' -printf '%f\n'
```

Selected raw output follows. The complete command logs and disk samples
are under `backfill/logs/2024-11/`. The search counts, disk reduction and
empty archive listing are in `backfill/2026-11-04-backfill-diagnosis.log`.

```text
downloaded/verified 6.64 GB in 878.8s
[15:03:59Z] snapshot /data/opencnpj-data/intelligence/backfill/work/2024-11/2024-11.ocnpj (6.644 GiB, sha256 c75c5e5cb6f87fd518b734246c56def67a7e486773198464b79c9b258b2a1ce4) in 579.7s: 60294489 companies, 63333654 documents, 165744 blocks, 26162154 terms
[15:07:11Z] sha256 ok: c75c5e5cb6f87fd518b734246c56def67a7e486773198464b79c9b258b2a1ce4
[15:08:39Z] empresas          source rows    60294481  snapshot rows    60294481  fingerprint MATCH
[15:08:39Z] estabelecimentos  source rows    63333645  snapshot rows    63333645  fingerprint MATCH
[15:08:39Z] socios            source rows    24933519  snapshot rows    24933519  fingerprint MATCH
[15:08:39Z] simples           source rows    41093715  snapshot rows    41093715  fingerprint MATCH
[15:08:39Z] rejected lines: source 0, snapshot 0 (+0 in reference tables)
[15:08:39Z] verify passed in 279.3s
Error: unexpected uf term "BR"
exit status: 1
indexed_uf_br_total 1
indexed_uf_br_active 0
sampled_peak_working_bytes 21734604800
sampled_minimum_volume_free_bytes 79862812672
```

The initial aggregate UF encoding accepted the federative abbreviations
and `EX`, with a reserved value for missing UF. The indexed `BR` row
belongs to the all-stock population for T2.

### Corrected build [MEASURED]

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo build --release -p opencnpj-lab
backfill=/data/opencnpj-data/intelligence/backfill
snapshot="$backfill/work/2024-11/2024-11.ocnpj"
catalogue=/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv
out="$backfill/checks/2024-11-br.ocagg"
nice -n 19 ionice -c3 "$CARGO_TARGET_DIR/release/opencnpj-intel" agg build --snapshot "$snapshot" --catalog "$catalogue" --out "$out" --tables T1,T2
nice -n 19 ionice -c3 "$CARGO_TARGET_DIR/release/opencnpj-intel" diff-agg --snapshot "$snapshot" --catalog "$catalogue" --agg "$out" --sample 10000 --seed 42
```

Raw output from `backfill/2026-11-04-br-recheck.log`:

```text
competence 2024-11, establishments 63333645, 11682020 bytes, SHA-256 c0a62f52dcd920895f453d63369a6d79a6594873211245cf539dd1beb4a3a17b
T1: 2983962 cells, 24597405 counts
T2: 4304763 cells, 63333645 counts
missing registration dates: 0, missing closure dates: 0
Cnae(T1): 1337 groups, 24597405 counts, match the engine
Uf(T1): 28 groups, 24597405 counts, match the engine
Municipality(T1): 5571 groups, 24597405 counts, match the engine
T1: 5000 sampled cells match the engine
Cnae(T2): 1342 groups, 63333645 counts, match the engine
Uf(T2): 29 groups, 63333645 counts, match the engine
Municipality(T2): 5571 groups, 63333645 counts, match the engine
T2: 5000 sampled cells match the engine
seed 42, 10000 sampled cells, 0 mismatches
```

## Completed archive [MEASURED]

The runner produced an aggregate, SHA-256 sidecar and source verification
manifest for every competence in the D-04 window. The October 2026 active
count matches the independent WP-1.4 build. The sanity check flags a
decrease or a monthly increase above 2%; the January 2025 and June–July
2026 changes are documented below.

The first line of blocks plots monthly active totals in competence order
on a linear scale between the printed minimum and maximum. The rows give
the exact counts, changes and sampled disk usage.

Raw output of `python3 -B crates/opencnpj-lab/scripts/backfill-aggregates.py --summarize`
is in `backfill/2026-11-06-archive-summary.log`.

```text
National T1 (24597405 to 28992547):
▁▁▁▂▂▃▃▃▄▄▅▅▆▆▆▇▇██▇▆▇▇▇
competence	active	all	active_change_pct	sampled_peak_working_bytes
2024-11	24597405	63333645	-	21734604800
2024-12	24874653	63723860	+1.127%	23067844608
2025-01	24653774	64017368	-0.888%	23331848192
2025-02	25053273	64547773	+1.620%	23373508608
2025-03	25441079	65168570	+1.548%	23522766848
2025-04	25686122	65565005	+0.963%	23806869504
2025-05	25858888	65847662	+0.673%	23997624320
2025-06	26152395	66349375	+1.135%	23876743168
2025-07	26375659	66723557	+0.854%	24139567104
2025-08	26621225	67125899	+0.931%	23992184832
2025-09	26941269	67640763	+1.202%	24596467712
2025-10	27191679	68048886	+0.929%	24694366208
2025-11	27428981	68448345	+0.873%	24742875136
2025-12	27647482	68879494	+0.797%	24904015872
2026-01	27786988	69200268	+0.505%	24801198080
2026-02	28186448	69882330	+1.438%	25553108992
2026-03	28470129	70386342	+1.006%	25606602752
2026-04	28739638	70864002	+0.947%	25869574144
2026-05	28992547	71314053	+0.880%	15567298560
2026-06	28092890	71874448	-3.103%	26217406464
2026-07	27800285	72318968	-1.042%	26194849792
2026-08	28148920	72789638	+1.254%	26412183552
2026-09	28263121	73366147	+0.406%	26708008960
2026-10	28380653	73809379	+0.416%	26746019840
verified_competences 24
sampled_peak_working_bytes 26746019840
sampled_minimum_volume_free_bytes 61612408832
```

### January 2025 change [MEASURED]

Active establishments fell from 24,874,653 to 24,653,774 across the
December and January snapshots. T2 independently gives the same active
totals and shows the closed stock rising from 28,796,347 to 29,435,813.
The official ZIP member names identify the source cuts as 2024-12-14 and
2025-01-11. The October 2026 snapshot still shows 470,980 closed
establishments dated 2024-12-31, including 467,533 classified as voluntary
liquidation. Its closed stock contains 643,595 records dated between the
two cuts, consistent with the increase of 639,466 in the verified monthly
archives. The later snapshot provides a retrospective cross-check of the
closure dates. The two stock totals alone do not identify individual
transitions.

```bash
backfill=/data/opencnpj-data/intelligence/backfill
deps=/data/opencnpj-target/release/deps
rustc -O --edition=2024 -L "dependency=$deps" --extern "opencnpj_insights=$deps/libopencnpj_insights-3555b8383e83f328.rlib" "$backfill/2026-11-04-check-stock-composition.rs" -o "$backfill/check-stock-composition"
nice -n 19 ionice -c3 "$backfill/check-stock-composition" "$backfill"/aggregates/2024-{11,12}.ocagg "$backfill"/aggregates/2025-01.ocagg
python3 -B "$backfill/2026-11-04-check-release-cutoffs.py"
rustc -O --edition=2024 -L "dependency=$deps" --extern "opencnpj_store=$deps/libopencnpj_store-1fde21f7fe4463b1.rlib" "$backfill/2026-11-04-check-closure-reasons.rs" -o "$backfill/check-closure-reasons"
nice -n 19 ionice -c3 "$backfill/check-closure-reasons" /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj
```

Raw output is in `backfill/2026-11-04-stock-composition.log`,
`backfill/2026-11-04-release-cutoffs.log` and
`backfill/2026-11-04-closure-reasons.log`. Selected lines follow.

```text
month,t2_missing,t2_01,t2_02,t2_03,t2_04,t2_08,t1_mei_missing,t1_mei_n,t1_mei_s,t1_simples_missing,t1_simples_n,t1_simples_s
2024-12,0,95102,24874653,262183,9695575,28796347,4893330,8022699,11958624,4893330,1745695,18235628
2025-01,0,95493,24653774,263314,9568974,29435813,4469900,9127819,11056055,4469900,3351051,16832823
2024-12 bytes 1525999969-1526016352/1526016353 K3241.K03200Y0.D41214.ESTABELE
2025-01 bytes 1547363380-1547379763/1547379764 K3241.K03200Y0.D50111.ESTABELE
2024-12-15,2024-12-30,85591,01:83173:EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA,75:1787:OBITO DO MEI - TITULAR FALECIDO,37:386:BAIXA DE PRODUTOR RURAL,02:167:INCORPORACAO,05:45:ENCERRAMENTO DA FALENCIA
2024-12-31,2024-12-31,470980,01:467533:EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA,75:3067:OBITO DO MEI - TITULAR FALECIDO,37:206:BAIXA DE PRODUTOR RURAL,02:168:INCORPORACAO,05:6:ENCERRAMENTO DA FALENCIA
2025-01-01,2025-01-11,87024,01:85145:EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA,75:1168:OBITO DO MEI - TITULAR FALECIDO,37:398:BAIXA DE PRODUTOR RURAL,02:223:INCORPORACAO,05:57:ENCERRAMENTO DA FALENCIA
```

**ESTIMATE.** The first month's download, build and source verification
took about 29 minutes combined (see the raw timings above). At that rate
the remaining 23 competences would take about 11 hours in sequence,
excluding retry delays.

### June–July 2026 status reclassification [MEASURED]

Active establishments fell from 28,992,547 in 2026-05 to 28,092,890 in
2026-06 and 27,800,285 in 2026-07, then recovered to 28,148,920 in
2026-08. The T2 status marginals identify a source-side movement into
status `04` (inapta): status `04` rose from 8,419,756 to 9,547,457 and
10,042,936 while status `02` fell by 899,657 and 292,605 and status `08`
(baixada) rose by 329,723 and 238,705. Total establishments kept rising
(+560,395 and +444,522), so no rows were lost. The August marginals show
a partial reversal: status `04` fell by 116,409 and active establishments
rose by 348,635.

```bash
backfill=/data/opencnpj-data/intelligence/backfill
nice -n 19 ionice -c3 "$backfill/check-stock-composition" "$backfill"/aggregates/2026-{05,06,07,08}.ocagg
```

```text
month,t2_missing,t2_01,t2_02,t2_03,t2_04,t2_08,t1_mei_missing,t1_mei_n,t1_mei_s,t1_simples_missing,t1_simples_n,t1_simples_s
2026-05,0,108838,28992547,312487,8419756,33480425,4742046,10801943,13448558,4742046,3490595,20759906
2026-06,0,109337,28092890,314616,9547457,33810148,4539299,10036857,13516734,4539299,2782032,20771559
2026-07,0,109673,27800285,317221,10042936,34048853,4508444,9833882,13457959,4508444,2581711,20710130
2026-08,0,109877,28148920,318161,9926527,34286153,4556214,9937684,13655022,4556214,2584202,21008504
```

### Host suspension and revised duration [MEASURED]

The host suspended at 2026-11-04 17:00:50 UTC-04 and resumed at
2026-11-05 06:46:43 UTC-04. That interval is 49,553 seconds. The November
2025 download spanned this suspension. Its successful download command
reported 1,018.3 seconds of running time. The runner continued afterward
and had completed 14 competences when the duration audit ran.

The median completed command durations were 777.5 seconds for download,
519.0 seconds for build and 287.4 seconds for source verification. The
median per-month sum was 1,716.8 seconds. The audit takes the largest
completed duration per stage when a month has multiple log entries.
These figures describe completed command execution and exclude host
suspension.

**ESTIMATE.** Ten remaining competences at that median rate require
4.77 hours, excluding additional suspend periods, retry delays and the
final delivery gate. January 2026 was already in progress at the audit.

A temporary `systemd-inhibit` sleep/idle lock now follows the existing
runner PID. Its process ends when that runner exits.

```bash
backfill=/data/opencnpj-data/intelligence/backfill
python3 -B "$backfill/2026-11-05-stage-timings.py"
journalctl -u systemd-suspend.service --since '2026-11-04 16:30:00' --until '2026-11-05 07:15:00' --no-pager -o short-iso
pid=$(pgrep -f '^python3 -B crates/opencnpj-lab/scripts/backfill-aggregates.py$')
setsid -f systemd-inhibit --what=idle:sleep --mode=block --who='OpenCNPJ WP-1.5' --why='Complete verified monthly aggregate backfill' tail --pid="$pid" -f /dev/null
systemd-inhibit --list --no-pager | rg 'OpenCNPJ WP-1.5'
```

Raw output and the timing script are under `backfill/`. The output log
is `2026-11-05-stage-timings.log`. Selected output follows.

```text
median_download_seconds 777.5
median_build_seconds 519.0
median_verify_seconds 287.4
completed_competences 14
median_pipeline_seconds 1716.8
remaining_competences 10
remaining_hours_estimate 4.77
documented_suspend_seconds 49553
2026-11-04T17:00:50-04:00 pop-os systemd-sleep[4073937]: Performing sleep operation 'suspend'...
2026-11-05T06:46:43-04:00 pop-os systemd-sleep[4073937]: System returned from sleep operation 'suspend'.
```

### Descriptor exhaustion and additional suspension [MEASURED]

The first resumed 2026-05 build made no progress after parsing 2 GiB of
CSV. The process held exactly 1,024 file descriptors, matching the soft
`RLIMIT_NOFILE` inherited by the runner, and every worker thread was
asleep. Relaunching with `ulimit -Sn 65536` completed the build in 848.1 s.
A later resume reran source verification after a partially written
`disk.tsv` sample broke the runner's parser; the snapshot was reused, so
no month was rebuilt. The 2026-05 sampled peak therefore covers the
verification and aggregate stages, while the global peak of 26.75 GB is
from 2026-10. The host suspended again on 2026-11-06 from 13:17:57 to
17:31:09 UTC-04 during the 2026-10 build; the build resumed and completed
afterwards.

### October 2026 cross-check [MEASURED]

The archived 2026-10 aggregate was compared cell by cell with the WP-1.4
artifact at `/data/opencnpj-data/intelligence/agg/2026-10.ocagg`. Every
T1 and T2 section reported zero mismatches.

```bash
backfill=/data/opencnpj-data/intelligence/backfill
"$backfill/2026-11-06-compare-t12" /data/opencnpj-data/intelligence/agg/2026-10.ocagg "$backfill/aggregates/2026-10.ocagg"
```

```text
T1 Cells(T1): 3189956 cells left, 3189956 cells right, 0 mismatches
T1 Cnae(T1): 1336 cells left, 1336 cells right, 0 mismatches
T1 Uf(T1): 28 cells left, 28 cells right, 0 mismatches
T1 Municipality(T1): 5572 cells left, 5572 cells right, 0 mismatches
T1 UfCnae(T1): 31423 cells left, 31423 cells right, 0 mismatches
T2 Cells(T2): 4567304 cells left, 4567304 cells right, 0 mismatches
T2 Cnae(T2): 1343 cells left, 1343 cells right, 0 mismatches
T2 Uf(T2): 28 cells left, 28 cells right, 0 mismatches
T2 Municipality(T2): 5572 cells left, 5572 cells right, 0 mismatches
T2 UfCnae(T2): 33867 cells left, 33867 cells right, 0 mismatches
```

### Archive validation [MEASURED]

All 24 archives match their JSON manifests, SHA-256 sidecars and embedded
manifests. The catalogue SHA-256, source verification status, aggregate
size, establishment totals and missing-date counters also agree.

```bash
python3 -B /data/opencnpj-data/intelligence/backfill/2026-11-06-validate-archives.py
```

```text
competences 24
mismatches 0
```

## Delivery gate

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three commands passed after the UF correction and again after the
complete backfill. Raw output is in
`backfill/2026-11-06-wp-1.5-delivery-gate.log`. The regression fixture
contains a non-active establishment with UF `BR` and checks its T2
marginal.
