# Benchmarks

Everything here was measured on the national release **2026-10** (RFB snapshot `D61010`,
37 ZIPs, 7,808,752,942 bytes; 28,856,680,141 bytes of CSV; 223,628,880 rows). Nothing is
extrapolated. Raw outputs (JSON/CSV/logs) are listed in [Reproduce](#reproduce).

## Format 1.2: the complete database (current)

Everything in this section was measured on the final binary (commit `2b68a94` plus
docs) against `2026-10-balanced-1.2.ocnpj`: balanced profile with every index in this
release — names, CPF (MEI/EI owners), partner documents, CNAE (principal/secundária),
CEP, DDD, contacts, filters — plus doc values and numeric columns. The sections further
down describe the 1.0 snapshots and the experiments that led here.

### Build and integrity

| | |
|---|---|
| Command | `opencnpj build --input raw/2026-10 --profile balanced --keep-partition` (end to end from the ZIPs) |
| Wall time | **8 min 2 s** (phase A 40.0 s, block encoding 343 s, index merge 71 s, numeric columns 11 s) |
| Peak RSS / temp disk | 5.95 GB / 18.3 GB |
| File | **8,333,664,200 bytes** (SHA-256 `76f36078…6037b`) |
| `verify --sources` | passed: all 223,628,880 rows match the ZIPs per table; every block, section and numeric value checked (4 min 41 s) |

Size by section (MB) — 1.0 balanced → 1.2:

| section | 1.0 | 1.2 | added |
|---|---:|---:|---:|
| data (blocks) | 4,999 | 5,034 | +35 (dictionary training variance) |
| postings | 1,343 | 1,899 | +556 (CEP 203, CPF 109, partner documents 103, DDD 68, motivo 35, contacts 18) |
| terms (FST) | 38 | 162 | +124 (17.3 M owner CPFs) |
| docvalues | – | 222 | município + CNAE ids |
| numeric | – | 1,007 | capital 294, opening date 136, status date 136, Simples opt-in/exclusion 120 + 120, MEI opt-in/exclusion 99 + 103 |
| other | 11 | 11 | |
| **total** | **6,391** | **8,334** | **+1,943** |

`capital_social` costs 31.6 bits/document because a few very large values widen whole
1,024-document blocks; patched frame-of-reference (outliers stored apart) would roughly
halve it and is the next storage optimization.

### Queries (in process, 500 sampled queries per category, seed 42)

| category | p50 / p95 / p99 ms (1 thread, warm) | q/s @1 / 4 / 16 threads | cold p50 / p95 ms |
|---|---:|---:|---:|
| lookup_cnpj | 0.166 / 0.200 / 0.223 | 5793 / 20777 / 48317 | 0.674 / 1.289 |
| lookup_cnpj_root | 0.156 / 0.216 / 0.297 | 5949 / 22495 / 54135 | 0.659 / 1.215 |
| lookup_cpf (owner + masked partners) | 7.157 / 11.897 / 12.120 | 134 / 500 / 1097 | 40.263 / 84.795 |
| filter_cnpj_prefix (6-char root prefix) | 0.912 / 1.261 / 1.333 | 1090 / 4115 / 8893 | 1.312 / 1.676 |
| name_full | 15.646 / 36.882 / 53.469 | 58 / 190 / 379 | 22.550 / 57.934 |
| name_popular | 14.697 / 40.159 / 65.421 | 50 / 173 / 406 | 14.832 / 62.218 |
| name_prefix_typeahead | 13.187 / 75.735 / 176.387 | 43 / 153 / 298 | 24.075 / 87.933 |
| name_fuzzy | 18.111 / 53.842 / 86.451 | 45 / 155 / 314 | 55.564 / 125.306 |
| name_plus_uf | 6.384 / 22.169 / 30.800 | 117 / 428 / 959 | 29.491 / 55.870 |
| filter_selective (município + CNAE + ativa) | 4.385 / 9.198 / 13.212 | 210 / 770 / 1801 | 14.804 / 26.267 |
| filter_broad (UF + ativa) | 4.569 / 10.091 / 10.639 | 175 / 602 / 1059 | 7.053 / 13.657 |
| filter_mei_active_municipio | 5.626 / 35.578 / 36.875 | 102 / 373 / 799 | 20.281 / 33.398 |
| sort_newest_in_municipio | 8.066 / 40.295 / 42.333 | 86 / 319 / 689 | 14.241 / 65.250 |
| filter_opened_in_year_uf | 74.959 / 171.207 / 177.770 | 12 / 43 / 87 | 77.103 / 159.877 |
| filter_cnae_division_uf_active (CNAE principal **or** secundária by division) | 29.487 / 809.690 / 877.745 | 8 / 26 / 58 | 35.958 / 758.922 |
| stats_municipio_by_uf | 22.663 / 90.927 / 90.927 | 32 / 77 / 105 | 45.981 / 90.249 |
| stats_cnae_by_uf | 23.977 / 90.535 / 90.535 | 31 / 77 / 97 | 47.647 / 93.760 |
| stats_uf_by_cnae | 86.327 / 207.438 / 212.542 | 10 / 36 / 91 | 70.841 / 207.188 |
| stats_year_opened_by_uf | 90.640 / 236.049 / 236.049 | 9 / 24 / 39 | 90.353 / 234.255 |
| stats_cnae_secundaria_by_uf | 2231.684 / 3361.536 / 3361.536 | 1 / 1 / 3 | 2613.117 / 3201.995 |

Snapshot open: 109 ms warm, 462 ms cold. Each query includes record fetch and JSON.

### HTTP (`opencnpj serve --data-dir`, oha 15 s, 16 connections, loopback)

| workload | req/s | p50 ms | p95 ms | p99 ms |
|---|---:|---:|---:|---:|
| lookup_cnpj | 36,442 | 0.40 | 0.68 | 0.93 |
| lookup_cnpj (64 connections) | 33,773 | 1.61 | 3.93 | 5.79 |
| lookup_cpf | 893 | 15.59 | 36.26 | 76.13 |
| filter_selective | 1,307 | 10.79 | 25.48 | 37.47 |
| filter_mei_active_municipio | 540 | 16.48 | 84.14 | 234.97 |
| sort_newest_in_municipio | 652 | 18.67 | 81.03 | 95.13 |
| name_prefix_typeahead | 362 | 29.08 | 127.32 | 286.65 |
| name_full | 184 | 73.82 | 175.83 | 306.45 |
| filter_opened_in_year_uf | 76 | 191.35 | 418.08 | 545.68 |
| stats_year_opened_by_uf | 54 | 241.58 | 542.69 | 612.29 |
| filter_cnae_division_uf_active | 58 | 79.03 | 1,342.40 | 2,048.85 |

Every response was HTTP 200. Start to `/readyz`: 0.12 s with 122 MB anonymous RSS;
after the whole run, 4.0 GB anonymous + 3.6 GB file-backed (page cache of the mapping).
Under concurrent name and filter load the large temporary bitmaps and the 512 MB postings
cache dominate anonymous memory; the allocator follow-up in the 1.0 section still applies.

### Optimizations measured on this database

Each row compares the same category (same seed and queries) before and after one change,
1 thread, warm:

| change | category | before p95 ms | after p95 ms |
|---|---|---:|---:|
| numeric probe/scan by candidate count, ordered appends, single-value reads | filter_mei_active_municipio | 1,080.9 | 35.6 |
| same | filter_opened_in_year_uf | 314.3 (p50 281.8) | 171.2 (p50 75.0) |
| top-k by block min/max with early stop | sort_newest_in_municipio | 96.8 | 40.3 |
| day→year table, run-length accumulation | stats_year_opened_by_uf | 533.2 | 236.0 |
| partner companies fetched without branches, in chunks | lookup_cpf (p99) | 92.9 | 12.1 |
| doc values for município/CNAE (format 1.1) | stats_municipio_by_uf | 2,796 (p50) | 22.7 (p50) |

### Known limits of this release

* `group_by=cnae_secundaria` takes 2–3 s (multi-valued; no doc values: 126.6 M
  document/value pairs would cost ~174 MB).
* CNAE prefixes over *secondary* CNAEs (`cnae_secundaria=47`, `cnae_qualquer=47`) union
  ~100 large postings lists: p50 29 ms but p95 0.8 s when they miss the postings cache.
* Name typeahead p99 ~180 ms for very frequent 3-letter prefixes.
* The < 5 GB target remains out of reach with all 1.2 indexes (8.33 GB balanced); the
  1.0 compact snapshot without them is 5.75 GB.

## Environment

| | |
|---|---|
| CPU | Intel Core i7-13620H (6P + 4E cores, 16 threads) |
| RAM | 31 GiB (+ zram swap) |
| Disk | Kingston NV3 1 TB NVMe (data), ext4 |
| OS | Pop!_OS 24.04, Linux 7.2.6 |
| Toolchain | rustc 1.99.0, zstd 1.5.7, `--release` (thin LTO) |
| Binary | built from commit `c45076c` (query code identical to the PR head except root-lookup pagination, re-measured below) |

**Caveat:** this is a shared developer desktop, not an isolated benchmark host. Load
average from other applications was 5–7 during several runs, so single-digit-millisecond
differences between runs are within noise. Thread scaling numbers include that contention.

## Integrity first

No result below counts unless the snapshot passed `opencnpj verify --sources`, which
re-reads all 37 ZIPs and compares the per-table row multiset (count + 64-bit hash sum)
with the rows decoded from the snapshot, after checking every block and section checksum
and the file SHA-256. All three national profiles passed:

| table | source rows | snapshot rows | fingerprint |
|---|---:|---:|---|
| empresas | 70,517,438 | 70,517,438 | match |
| estabelecimentos | 73,809,379 | 73,809,379 | match |
| socios | 28,495,961 | 28,495,961 | match |
| simples | 50,806,102 | 50,806,102 | match |
| rejected lines | 0 | 0 | — |

Also observed: 2 companies without an `empresas` row, 1 duplicated `empresas` key, 0
duplicated establishment keys — all kept and counted in the manifest.

## Build (national)

`wall s`: balanced is end to end from the ZIPs (phase A included, 70.7 s);
fast and compact were rebuilt from the kept phase-A output (`--from-partition`), so add
~71 s for an end-to-end figure. Peak temp is the largest on-disk working set
(bucket files + segments + runs), RSS is `VmHWM`.

| profile | file GB | payload GB | postings GB | terms MB | other MB | encode s | index merge s | wall s | peak RSS GB | peak temp GB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| fast | 7.63 | 6.12 | 1.34 | 37.7 | 124.4 | 177 | 33 | 228 | 3.45 | 18.0 |
| balanced | 6.39 | 5.00 | 1.34 | 37.7 | 10.7 | 420 | 40 | 546 | 5.24 | 16.8 |
| compact | 5.75 | 4.37 | 1.34 | 37.7 | 3.3 | 547 | 52 | 631 | 3.04 | 16.2 |

Phase A (inflate + parse + partition 26.87 GiB of CSV) measured 38.1 s in the first run
(ZIPs in page cache) and 70.7 s in the final one (ZIPs evicted, desktop under load):
0.4–0.7 GB/s, 3.2–5.9 M rows/s. With a cheaper codec the whole build is short: the
first national build (64 KiB, zstd-9, before index refinements) took **3 min 47 s**
end to end (`/usr/bin/time`: 2,128 s user, 955% CPU, 5.1 GB max RSS).

### Versus the legacy PostgreSQL importer

| | legacy (2026-06, `docs/legacy/benchmarks`) | OpenCNPJ (2026-10) |
|---|---|---|
| Rows | 217,259,823 | 223,628,880 |
| Import + indexes | 27 min 9 s | 3 min 47 s (zstd-9 64 KiB) – 10 min 31 s (compact) |
| Throughput | ~167k rows/s | 0.32 M rows/s (compact, 702 s) – 0.98 M rows/s (first build, 227 s), end to end |
| Storage | PostgreSQL data ~35–45 GB + indexes ~15–20 GB (*legacy docs estimate*, not measured here) | 5.75–7.63 GB, one file, indexes included |
| Rejected rows | ~116,746 parse/skip errors, not stored | 0 rejects; any reject would be stored |

The scopes differ: the legacy run loaded a June release into a tuned PostgreSQL
(B-tree, trigram and FTS indexes, statistics) with deduplication; this pipeline also
hashes sources, compresses, and builds its own inverted index, but runs no database.
Same machine model, different months and software, so read the comparison as an order of
magnitude, not a controlled A/B.

## Storage comparison

| representation | bytes | searchable | notes |
|---|---:|---|---|
| A. official ZIPs | 7,808,752,942 | no | byte-for-byte source (keep for forensics) |
| B. extracted CSV | 28,856,680,141 | no | `unzip` took 140 s |
| C. Parquet + ZSTD-9 (DuckDB 1.5.4) | 5,176,269,524 | scans only | all columns as text, no index |
| D. SQLite 3.45 + B-tree + FTS5 | 37,866,299,392 | yes | tables 24.5 GB, + B-tree 7.6 GB, + FTS5 5.8 GB |
| E. OpenCNPJ compact | 5,752,780,265 | yes | 1 MiB blocks, zstd-19 |
| F. OpenCNPJ fast | 7,626,519,835 | yes | 4 KiB blocks, zstd-9 + dict |
| G. OpenCNPJ balanced | 6,390,868,664 | yes | 64 KiB blocks, zstd-15 + dict |

OpenCNPJ sizes include every index, dictionary and the manifest. GB figures elsewhere are decimal (10⁹).

Baselines were built from the same ZIPs by `scripts/baselines.sh` (all columns as text,
so the semantic content matches). DuckDB parsed exactly the same row counts as OpenCNPJ
for all four tables (70,517,438 / 73,809,379 / 28,495,961 / 50,806,102), an independent
check of the parser. Its parallel CSV reader rejects these files (unescaped quotes) and
its Latin-1 reader rejects some valid ISO-8859-1 bytes, so the script transcodes with
`iconv` and reads single-threaded.

| step | seconds |
|---|---:|
| unzip → CSV (B) | 140 |
| iconv Latin-1 → UTF-8 (DuckDB requirement) | 73 |
| CSV → Parquet ZSTD-9, 4 tables (C) | 234 |
| Parquet → SQLite tables | 237 |
| SQLite B-tree indexes (6) | 304 |
| SQLite FTS5 over razão social + nome fantasia (74 M rows) | 677 |

Lookup and name-search latency on the baselines, same sampled CNPJs and names as the
engine benchmark, single pass:

| system / workload | queries | p50 ms | p95 ms | p99 ms |
|---|---:|---:|---:|---:|
| SQLite: CNPJ lookup (4 indexed queries) | 300 | 19.840 | 36.165 | 48.983 |
| SQLite: full name, FTS5 count + top 20 | 300 | 338.334 | 866.284 | 1483.943 |
| SQLite: typeahead prefix, FTS5 count + top 20 | 300 | 46.211 | 818.031 | 1814.972 |
| DuckDB on Parquet: CNPJ lookup (4 scans, 16 threads) | 10 | 271 (median) | — | max 285 |
| OpenCNPJ balanced: CNPJ lookup + JSON | 500 | 0.158 | 0.189 | 0.214 |
| OpenCNPJ balanced: full name + fetch + JSON | 500 | 16.936 | 40.443 | 57.955 |

The SQLite database (37.9 GB) does not fit in the free page cache of this machine, which
is part of its cost; `scripts/baseline_queries.py` reproduces these numbers.

Reading the comparison:

* **Against Parquet + ZSTD** (the strongest compression baseline, no index at all), the
  OpenCNPJ payload is smaller in every profile (4.37 / 5.00 / 6.12 GB vs 5.18 GB) — the
  compact *and* balanced payloads beat it — while being randomly accessible per company.
  Including the full inverted index, compact is 11% larger than index-less Parquet.
* **Against SQLite** with indexes for the same access paths, compact is 6.6× smaller
  (balanced 5.9×), and balanced CNPJ lookups are ~125× faster at p50 (compact ~13×); FTS5 has no ranking by name length, no fuzzy
  matching and costs 5.8 GB on its own.
* **Against the ZIPs**, the searchable compact snapshot is 0.74× the size of the
  download.

The aspirational < 5 GB target was **not reached**: the smallest complete, searchable
snapshot is 5.75 GB (compact). Payload alone is 4.37 GB; the inverted index adds
1.38 GB. Where the index bytes go (balanced, `opencnpj info --index`):

| field | terms | postings MB |
|---|---:|---:|
| razao_social | 3,113,708 | 524.4 |
| cnae_secundaria | 1,350 | 179 |
| nome_fantasia | 2,447,814 | 154.2 |
| municipio | 5,572 | 121 |
| cnae | 1,343 | 101 |
| uf | 28 | 64 |
| razao_len + fantasia_len (ranking) | 31 | 72 |
| others (situação, porte, natureza, simples, mei, matriz) | 105 | 121 |

Filter postings are large because documents are ordered by CNPJ (to keep companies
contiguous), so a UF or município has no locality. Getting under 5 GB would need either a
different document order for the index or dropping secondary-CNAE/município postings
in favour of a forward column — both are trade-offs against lookup locality or filter
speed and were not adopted.

## Experiment matrix (10% deterministic sample)

Every variant rebuilt from the same phase-A output, restricted to buckets with
`id % 10 == 0` (7,355,219 documents), verified, then 2,000 sampled CNPJ lookups
(1 thread, warm). `payload` is the block data; `file` additionally includes an index
built with an earlier tokenizer (CPF tokens indexed), so compare payloads.

| variant | codec | block | dict | typed cols | payload MB | file MB | encode s | lookup p50 ms | p95 ms | p99 ms |
|---|---|---:|---:|---|---:|---:|---:|---:|---:|---:|
| z9-d112-b4 | zstd9 | 4 KiB | 112 KiB | yes | 609.3 | 798.3 | 11.5 | 0.032 | 0.046 | 0.062 |
| z9-d112-b16 | zstd9 | 16 KiB | 112 KiB | yes | 556.2 | 736.2 | 10.6 | 0.063 | 0.078 | 0.100 |
| z9-d112-b64 | zstd9 | 64 KiB | 112 KiB | yes | 529.7 | 707.5 | 10.4 | 0.176 | 0.257 | 0.300 |
| z9-d112-b256 | zstd9 | 256 KiB | 112 KiB | yes | 507.2 | 684.4 | 8.9 | 0.524 | 0.618 | 0.670 |
| z9-d112-b1024 | zstd9 | 1024 KiB | 112 KiB | yes | 461.1 | 638.1 | 8.5 | 1.825 | 2.169 | 2.276 |
| none-d112-b64 | none | 64 KiB | 112 KiB | yes | 1282.0 | 1459.6 | 6.0 | 0.082 | 0.108 | 0.130 |
| lz4-d112-b64 | lz4 | 64 KiB | 112 KiB | yes | 805.6 | 983.2 | 5.8 | 0.115 | 0.157 | 0.193 |
| zstd1-d112-b64 | zstd1 | 64 KiB | 112 KiB | yes | 639.4 | 817.2 | 6.7 | 0.151 | 0.176 | 0.194 |
| zstd3-d112-b64 | zstd3 | 64 KiB | 112 KiB | yes | 582.4 | 760.2 | 6.4 | 0.172 | 0.200 | 0.228 |
| zstd6-d112-b64 | zstd6 | 64 KiB | 112 KiB | yes | 534.8 | 712.6 | 9.1 | 0.181 | 0.219 | 0.255 |
| zstd12-d112-b64 | zstd12 | 64 KiB | 112 KiB | yes | 522.6 | 700.4 | 18.8 | 0.204 | 0.335 | 0.375 |
| zstd15-d112-b64 | zstd15 | 64 KiB | 112 KiB | yes | 500.6 | 678.3 | 36.3 | 0.190 | 0.233 | 0.294 |
| zstd19-d112-b64 | zstd19 | 64 KiB | 112 KiB | yes | 498.0 | 675.8 | 40.5 | 0.205 | 0.239 | 0.262 |
| z9-d0-b16 | zstd9 | 16 KiB | 0 | yes | 653.4 | 833.4 | 9.1 | 0.052 | 0.070 | 0.095 |
| z9-d0-b64 | zstd9 | 64 KiB | 0 | yes | 578.1 | 755.8 | 8.3 | 0.142 | 0.172 | 0.216 |
| z9-d0-b256 | zstd9 | 256 KiB | 0 | yes | 539.4 | 716.5 | 8.5 | 0.454 | 0.548 | 0.613 |
| z3-d0-b16 | zstd3 | 16 KiB | 0 | yes | 685.7 | 865.7 | 6.5 | 0.052 | 0.065 | 0.077 |
| z9-d256-b64 | zstd9 | 64 KiB | 256 KiB | yes | 527.3 | 705.2 | 12.8 | 0.173 | 0.201 | 0.224 |
| z19-d0-b256 | zstd19 | 256 KiB | 0 | yes | 478.1 | 655.2 | 39.6 | 0.536 | 0.630 | 0.670 |
| z19-d112-b256 | zstd19 | 256 KiB | 112 KiB | yes | 464.4 | 641.6 | 28.8 | 0.568 | 0.668 | 0.721 |
| z19-d0-b1024 | zstd19 | 1024 KiB | 0 | yes | 437.1 | 614.0 | 34.2 | 1.903 | 2.255 | 2.356 |
| plain-z9-d112-b64 | zstd9 | 64 KiB | 112 KiB | **no** | 617.6 | 793.0 | 10.9 | 0.141 | 0.160 | 0.193 |
| plain-z19-d0-b256 | zstd19 | 256 KiB | 0 | **no** | 547.5 | 721.9 | 63.0 | 0.407 | 0.457 | 0.490 |

Findings:

* **Typed column codecs** (dates, money, digits, CNAE lists, dictionaries) cut the
  compressed payload by 14% (64 KiB) and 13% (256 KiB) versus plain text, and roughly
  shrink the uncompressed block data to 1.28 GB for 10% of the documents (the full
  release is 28.9 GB of CSV).
* **Trained zstd dictionary**: −15% at 16 KiB, −8% at 64 KiB, −6% at 256 KiB; 256 KiB of
  dictionary is no better than 112 KiB.
* **zstd level**: 9 → 15 saves 5.5% at 3.5× encode time; 15 → 19 saves 0.5% more.
  Decompression cost is flat across levels. LZ4 is 1.5× larger than zstd-9 for a lookup
  only ~0.06 ms faster.
* **Block size** is the main knob: each 4× step costs 2–3.5× lookup latency and saves
  4–9% payload.

### Pareto front and the chosen profiles

On (payload, lookup p95, encode time) the non-dominated points are 4 KiB/zstd-9+dict,
16 KiB/zstd-9+dict, 64 KiB/zstd-15+dict, 256 KiB/zstd-19+dict and 1 MiB/zstd-19. The
profiles take three of them:

| profile | choice | why |
|---|---|---|
| fast | 4 KiB, zstd-9, 112 KiB dict | lowest latency (lookup p50 0.03 ms) for +22% payload |
| balanced | 64 KiB, zstd-15, 112 KiB dict | within 6% of the larger-block payloads while search pages (20 records) stay cheap |
| compact | 1 MiB, zstd-19 | smallest payload measured; lookups ~1.4 ms, still far below the 10 ms goal |

## Queries (national, warm cache)

In-process (`opencnpj bench`): each query = search + fetch of up to 20 records + JSON
serialization, the same work as the API minus HTTP. 500 queries per category sampled
with seed 42 from the snapshot itself (random documents → their CNPJ, root, name,
UF, município, CNAE), one untimed warm-up pass, then 1 thread.

| category | fast p50 / p95 / p99 ms | balanced p50 / p95 / p99 ms | compact p50 / p95 / p99 ms | balanced q/s @1/4/16 threads |
|---|---:|---:|---:|---:|
| filter_broad | 4.715 / 10.411 / 10.641 | 5.087 / 11.151 / 11.848 | 4.865 / 10.285 / 10.571 | 157 / 534 / 944 |
| filter_selective | 2.430 / 7.205 / 11.324 | 5.186 / 9.873 / 13.892 | 33.831 / 39.269 / 43.055 | 181 / 647 / 1323 |
| lookup_cnpj | 0.033 / 0.055 / 0.076 | 0.160 / 0.206 / 0.269 | 1.376 / 1.772 / 1.952 | 5936 / 21859 / 54814 |
| lookup_cnpj_root | 0.031 / 0.059 / 0.234 | 0.156 / 0.210 / 0.390 | 1.347 / 1.799 / 2.107 | 2102 / 3612 / 3301 |
| name_full | 15.543 / 35.916 / 55.291 | 16.936 / 40.443 / 57.955 | 19.491 / 61.284 / 84.063 | 53 / 179 / 350 |
| name_fuzzy | 16.228 / 48.669 / 77.889 | 18.590 / 53.785 / 85.416 | 44.189 / 79.500 / 111.480 | 44 / 152 / 316 |
| name_plus_uf | 3.332 / 16.770 / 24.621 | 6.385 / 21.848 / 29.835 | 34.274 / 49.626 / 56.532 | 120 / 359 / 831 |
| name_popular | 12.185 / 36.394 / 63.421 | 16.303 / 45.889 / 73.760 | 42.586 / 67.928 / 86.409 | 46 / 112 / 129 |
| name_prefix_typeahead | 10.476 / 69.225 / 174.421 | 14.888 / 82.940 / 211.575 | 37.448 / 100.138 / 196.269 | 38 / 99 / 112 |
| stats_municipio_by_uf | 5130.849 / 8192.890 / 8192.890 | 5526.838 / 8676.454 / 8676.454 | 5409.770 / 8318.320 / 8318.320 | 0 / 1 / 1 |
| stats_uf_by_cnae | 89.891 / 214.241 / 221.477 | 89.962 / 213.886 / 221.013 | 85.355 / 203.821 / 211.704 | 10 / 35 / 88 |

`lookup_cnpj_root` above was measured before root lookups were paginated (it serialized
every branch of giants like Banco do Brasil, 7,903 establishments). Re-measured with the
final binary (default page of 100 establishments):

| profile | lookup_cnpj p50 / p95 / p99 ms | q/s @1/4/16 | lookup_cnpj_root p50 / p95 / p99 ms | q/s @1/4/16 |
|---|---:|---:|---:|---:|
| fast | 0.033 / 0.050 / 0.059 | 24652 / 83404 / 121726 | 0.034 / 0.059 / 0.169 | 22144 / 84646 / 177516 |
| balanced | 0.158 / 0.189 / 0.214 | 6021 / 23515 / 48478 | 0.172 / 0.217 / 0.287 | 5539 / 21767 / 48622 |
| compact | 1.502 / 1.923 / 2.036 | 669 / 2377 / 5152 | 1.532 / 2.128 / 2.521 | 644 / 2491 / 4898 |

#### Cold cache (balanced, page cache evicted before each category; open 621.1 ms)

| category | p50 ms | p95 ms | p99 ms |
|---|---:|---:|---:|
| filter_broad | 6.483 | 15.710 | 40.768 |
| filter_selective | 15.565 | 26.845 | 43.811 |
| lookup_cnpj | 0.698 | 1.358 | 1.469 |
| lookup_cnpj_root | 0.697 | 1.385 | 1.654 |
| name_full | 23.450 | 55.620 | 82.919 |
| name_fuzzy | 57.132 | 125.148 | 187.143 |
| name_plus_uf | 30.453 | 56.177 | 82.555 |
| name_popular | 15.296 | 63.802 | 82.162 |
| name_prefix_typeahead | 25.910 | 85.717 | 219.659 |
| stats_municipio_by_uf | 6131.047 | 8219.021 | 8219.021 |
| stats_uf_by_cnae | 70.400 | 204.279 | 219.516 |

Cold numbers: `posix_fadvise(DONTNEED)` on the snapshot, reopen, then the category with
1 thread; the first queries of each category read from NVMe. `open` includes mapping the
file and verifying header, footer, section table and the checksummed small sections.

## HTTP API (oha)

`opencnpj serve` on the balanced snapshot, `oha` replaying 2,000 sampled URLs per
workload for 20 s over loopback (client and server on the same 16 threads). Every
response was HTTP 200.

| workload | connections | req/s | p50 ms | p95 ms | p99 ms | success |
|---|---:|---:|---:|---:|---:|---:|
| filter_selective | 16 | 711 | 19.03 | 47.76 | 71.44 | 100.0% |
| filter_selective | 64 | 791 | 72.06 | 145.47 | 212.11 | 100.0% |
| lookup_cnpj | 16 | 32678 | 0.44 | 0.80 | 1.09 | 100.0% |
| lookup_cnpj | 64 | 33259 | 1.63 | 4.07 | 6.28 | 100.0% |
| name_prefix_typeahead | 16 | 237 | 43.03 | 197.64 | 402.67 | 100.0% |
| name_prefix_typeahead | 64 | 267 | 204.43 | 481.75 | 764.59 | 100.0% |

Process start to `/readyz` ready: 0.12 s.

### Server memory (balanced, `/proc/<pid>/status`)

| state | VmRSS | anonymous | file-backed (mmap of the snapshot) |
|---|---:|---:|---:|
| idle after start | 159 MB | 106 MB | 53 MB |
| after 10 s of CNPJ lookups (16 connections) | 238 MB | 169 MB | 69 MB |
| after 10 s of typeahead (16 connections) | 2.78 GB (peak 2.95 GB) | 1.72 GB | 1.06 GB |

Lookups need very little memory. Name search allocates large temporary bitmaps; the
decoded-postings cache is capped at 256 MB, so most of the anonymous growth is glibc
keeping freed memory in per-thread arenas. With `MALLOC_ARENA_MAX=2` the same load
ended at 0.91 GB anonymous / 1.64 GB total, but that run overlapped the DuckDB baseline,
so its throughput is not comparable. An allocator change is a follow-up that needs a
clean A/B on an idle host. File-backed pages are the OS page cache and are reclaimable.


Against the research goals: CNPJ lookup p95 ≤ 10 ms holds for every profile, cold or
warm (worst: compact warm p95 1.8 ms). Common text search p95 ≤ 100 ms holds for full
names, popular names, name + UF and fuzzy on fast/balanced; prefix typeahead is at
69–100 ms p95 with a long tail (p99 175–212 ms) from 3-letter prefixes of very frequent
words.

`stats_municipio_by_uf` in the table above was measured on format 1.0, where every
município's postings were intersected with the candidates. Format 1.1 removed that
bottleneck; see the next section.

### Group-by with doc values (format 1.1)

Format 1.1 adds a `docvalues` section (`docs/FORMAT.md`): one bit-packed value id per
document for município (13 bits) and CNAE principal (11 bits). `group_by` on those fields
counts the candidates in one pass instead of intersecting 5,572 / 1,343 postings lists.
Measured with one binary (commit with this change, `--release`) on the 1.0 balanced
snapshot (postings path: the reader falls back when the section is absent) and on a 1.1
balanced snapshot rebuilt from the same phase-A output and verified with
`verify --sources` (all four tables MATCH, 0 rejects, doc values cross-checked against
postings). `bench --only stats --queries 1000`, warm, 1 thread; group-bys filtered by one
UF use 20 sampled queries (seed 42, identical in every run), `stats_uf_by_cnae` 1,000.

| category | 1.0 p50 / p95 / p99 ms | 1.0 rerun p50 / p95 / p99 ms | 1.1 p50 / p95 / p99 ms |
|---|---:|---:|---:|
| stats_municipio_by_uf | 2795.951 / 7228.288 / 7997.579 | 2786.513 / 7316.567 / 8149.414 | 14.591 / 80.755 / 82.788 |
| stats_cnae_by_uf | 983.683 / 2232.179 / 2465.259 | 984.905 / 2169.640 / 2400.565 | 14.253 / 85.378 / 87.002 |
| stats_cnae_secundaria_by_uf | 1490.838 / 3117.249 / 3414.350 | 1494.187 / 3095.618 / 3427.662 | 1498.812 / 3040.676 / 3364.706 |
| stats_uf_by_cnae | 82.635 / 232.760 / 253.546 | — | 73.877 / 207.347 / 211.923 |

Município and CNAE group-bys are ~190× / ~69× faster at p50; the p95 (~81–85 ms) is
the largest UF (São Paulo), whose candidate bitmap the one-pass count iterates in full.
Secondary CNAE and `stats_uf_by_cnae` (UF has 28 values) do not use doc values and did not
change beyond run-to-run noise. Load average was 2.7–6.6 during these runs (the
`uptime` lines in the logs); the 1.0 rerun came right after the 1.1 runs.

Thread scaling on 1.1 (same workload):

| category | p50 ms @1/4/16 | q/s @1/4/16 |
|---|---:|---:|
| stats_municipio_by_uf | 15.029 / 15.351 / 33.409 | 29 / 89 / 148 |
| stats_cnae_by_uf | 14.209 / 15.501 / 28.838 | 29 / 89 / 146 |

**Size cost:** the `docvalues` section is 221,466,852 bytes (73,809,381 documents ×
24 bits + value lists + padding), +3.5% on the balanced file; `terms`, `postings` and
`dictionaries` are byte-identical to the 1.0 build. The 1.1 file is 6,641,347,055 bytes,
but 29.0 MB of the difference from the 1.0 file (6,390,868,664) is the `data` section:
the trained zstd dictionary differed between the two builds (its sample is gathered by
parallel workers up to a byte budget, so it depends on thread timing), not the doc
values. Building the column runs inside the index merge: `merge_index` took 46.9 s versus
39.7 s in the 1.0 build (different runs and load, not a controlled A/B); peak RSS of the
from-partition rebuild was 3.58 GB.

**Secondary CNAE stays on postings.** It is multi-valued: the 2026-10 snapshot has
126,557,313 (document, secondary CNAE) pairs over 34,936,591 documents (counted from the
postings). A list column would need ≥ 174 MB for 11-bit ids alone (126,557,313 × 11 bits,
arithmetic, not built) plus per-document offsets — more than both single-valued columns
together — to speed up one group-by that takes ~1.5 s today. Not adopted.

## Reproduce

```bash
B=target/release/opencnpj
$B download --dest data/raw --month 2026-10
$B build --input data/raw/2026-10 --output data/snapshots/2026-10-balanced.ocnpj \
   --tmp data/work/2026-10 --profile balanced --keep-partition --report data/reports/build-2026-10-balanced.json
for p in fast compact; do
  $B build --input data/raw/2026-10 --from-partition --tmp data/work/2026-10 --profile $p \
     --output data/snapshots/2026-10-$p.ocnpj --report data/reports/build-2026-10-$p.json
done
for p in balanced fast compact; do $B verify data/snapshots/2026-10-$p.ocnpj --sources data/raw/2026-10; done
# doc values (format 1.1): rebuild, verify, group-by before/after
$B build --input data/raw/2026-10 --from-partition --tmp data/work/2026-10 --profile balanced \
   --output data/snapshots/2026-10-balanced-1.1.ocnpj --report data/reports/build-2026-10-balanced-1.1.json
$B verify data/snapshots/2026-10-balanced-1.1.ocnpj --sources data/raw/2026-10
$B bench --snapshot data/snapshots/2026-10-balanced.ocnpj --queries 1000 --threads 1 --only stats --out data/bench/docvalues/before
$B bench --snapshot data/snapshots/2026-10-balanced-1.1.ocnpj --queries 1000 --threads 1 --only stats --out data/bench/docvalues/after
$B bench --snapshot data/snapshots/2026-10-balanced-1.1.ocnpj --queries 1000 --threads 1,4,16 --only stats --out data/bench/docvalues/after-threads
scripts/experiments.sh $B data/work/2026-10 data/experiments 10   # matrix
scripts/measure-all.sh $B data 2026-10                           # queries, cold, HTTP, baselines
python3 -I scripts/report.py data 2026-10                         # tables in this file
```

Profiling note: `perf`/`samply` were unavailable (`perf_event_paranoid=2`, no root), so
hotspots were located with phase timers in the build report and temporary per-phase
tracing of the query engine; every optimization listed in the commit history was kept
only after a before/after measurement on the national snapshot.
