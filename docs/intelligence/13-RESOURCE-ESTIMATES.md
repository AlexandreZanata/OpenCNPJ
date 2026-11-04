# 13 — Resource estimates (RAM, CPU, storage)

Labels: **[MEASURED]** has a command and raw output; **[ESTIMATE]** is derived from
measured inputs with the formula shown; **[HYPOTHESIS]** awaits the named work package.
No number here may be quoted as a result.

## 1. Deployment topology

```text
 users ──HTTPS──► Caddy ──► chat node (1 vCPU / 1 GB)                      data node (8–16 cores / 16 GB)
                            opencnpj chat-serve                             opencnpj serve (existing)
                            ├─ language pack + optional classifier          ├─ .ocnpj snapshot (mmap, 8.3 GB)
                            ├─ catalogues (municipalities, sectors, IBGE)   └─ /api/v1/search, /cnpj, /stats
                            ├─ aggregates .ocagg (current + history, mmap)        ▲
                            ├─ response cache keyed by canonical IR               │ lists, lookups, rare filters
                            └─ engine client (service key, bounded) ──────────────┘
```

Single-node mode (development and small deployments): the chat route runs inside
`opencnpj serve` and calls the engine in process.

## 2. Chat node memory

| Item | Size | Label | Basis |
|---|---:|---|---|
| Process baseline (tokio, axum, gate, metrics) | 20–40 MB | ESTIMATE | `serve` reached `/readyz` with 122 MB anonymous RSS *including* the open snapshot and its caches (BENCHMARKS.md); the chat node opens no snapshot and uses small caches |
| Language pack (FSTs, automaton, payloads) | < 5 MB | ESTIMATE | ~20k phrases (05-INTERPRETER.md §3) |
| Classifier (if shipped) | 0.3–2.4 MB | ESTIMATE | 11-MICRO-CLASSIFIER.md §5 |
| Catalogues in memory | 1–3 MB | ESTIMATE | 5,572 municipalities × ~200 B; ~200 sectors; 33.4k neighbour pairs × 4 B |
| Aggregates, current competence (T1–T5 + marginals) | 30–60 MB file-backed | ESTIMATE | 2.83 + 4.57 + 8.7 + 5.4 + 2.79 M cells [MEASURED] × ~2–3 B/cell compressed |
| Aggregates, 24 past competences (T1/T2 only) | 150–300 MB file-backed | ESTIMATE | (2.83 + 4.57) M cells × ~1–2 B × 24; only hot pages stay resident |
| Response cache | 64 MB (configurable) | design | moka, weighted by bytes |
| Per-worker scratch | < 64 KB × workers | ESTIMATE | token/lattice buffers reused |
| **Total anonymous** | **≈ 90–150 MB** | ESTIMATE | leaves > 600 MB for page cache in 1 GB |

Interpreter-only increment (NFR-04, < 20 MB): language pack + classifier + catalogues +
scratch ≈ 5–10 MB [ESTIMATE]; measured in WP-4.1/4.2 with `VmRSS` deltas.

## 3. Chat node CPU

| Step | Typical | Label | Basis |
|---|---:|---|---|
| Interpretation (S0–S11) | 10–100 µs | HYPOTHESIS | O(tokens) FST lookups + one automaton pass; Snips' full statistical NLU measured 1.26 ms on a laptop and 60 ms on a Raspberry Pi 3 (03 §2), and ours does less work per query |
| Aggregate point query (one sector, one city) | 5–50 µs | HYPOTHESIS | binary search in block directory + one block decode |
| Ranking of municipalities in a UF for one sector | 0.1–1 ms | HYPOTHESIS | ≤ 5,572 municipalities × ≤ 32 flag combinations per sector, usually far fewer cells |
| National ranking of sectors (marginals) | < 0.1 ms | HYPOTHESIS | ~1,343 CNAE marginals per competence |
| Worst case: full T1 scan | 3–10 ms | ESTIMATE | 2.83 M cells × 1–3 ns; avoided by the marginals |
| Render + JSON | 20–100 µs | HYPOTHESIS | templates over small results |
| **Aggregate-backed answer, server side** | **< 2 ms typical** | HYPOTHESIS | ⇒ ~500+ uncached answers/s on one vCPU |
| Engine-backed answer | engine latency + ~0.1–0.5 ms | ESTIMATE | engine figures in BENCHMARKS.md (e.g. `filter_selective` p95 9.2 ms in process, 25.5 ms over HTTP at 16 connections) |

Concurrency: "hundreds of connected clients" are mostly idle keep-alive connections
(a few KB each in tokio). CPU work runs behind a semaphore of 2 × vCPU; upstream engine
calls behind their own bound (default 16) with timeouts; overload answers `503 overloaded`
like the existing lanes.

## 4. Storage

| Where | Item | Size | Label |
|---|---|---:|---|
| Chat node | binary | ≤ 90 MB | MEASURED for today's binary: `/data/opencnpj-target/release/opencnpj` is 88,137,416 bytes (`ls -la`, with `debug = "line-tables-only"`); a stripped chat build is expected to be much smaller [ESTIMATE] |
| Chat node | language pack + classifier + catalogues | < 15 MB | ESTIMATE |
| Chat node | aggregates: current + 24 months | 200–400 MB | ESTIMATE |
| Build host | historical backfill, one competence in flight | ≈ 35 GB peak on `/data` | ESTIMATE from BENCHMARKS.md (7.8 GB ZIPs, 18.3 GB build scratch, 8.3 GB snapshot) |
| Build host | synthetic master dataset (300k items × ~300 B) | ~90 MB; learning-curve subsets are index files | ESTIMATE |
| Build host | `/` free space | 3.6 GB — do not use for builds | MEASURED (`df -h`, 2026-11-04) |

## 5. Build-time costs

| Job | Time | Label |
|---|---:|---|
| National snapshot build | 8 min 2 s, peak RSS 5.95 GB | MEASURED (BENCHMARKS.md) |
| Aggregates from a snapshot | < 5 min | HYPOTHESIS (WP-1.4); the DuckDB equivalents over Parquet took 0.2–8.5 s per table with 8 threads [MEASURED] |
| Historical backfill (24 months) | download time + ~24 × 15 min | ESTIMATE; download bandwidth unknown |
| Language pack compile | seconds | HYPOTHESIS |
| Classifier training (100k examples, linear) | seconds to minutes | HYPOTHESIS |

## 6. Cost per user

Not estimated yet: it must come from measured CPU-seconds per 1,000 questions (WP-6.6)
multiplied by the chosen VPS price (provider and date recorded):

```text
cost per 1,000 questions = (measured CPU-seconds per 1,000 questions / 3,600) × price per vCPU-hour
```

Engine-backed questions add the data node's cost, which DEPLOYMENT.md already bounds per
plan (rate limits, quotas, response cache).
