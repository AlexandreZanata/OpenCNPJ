# Architecture

OpenCNPJ turns the monthly Receita Federal CNPJ release into one immutable, compressed,
indexed file (`.ocnpj`) and serves it with a single binary. There is no database, cache
server or search cluster. Measured results are in [BENCHMARKS.md](BENCHMARKS.md); the
byte layout is in [FORMAT.md](FORMAT.md).

## Crates

| Crate | Responsibility |
|-------|----------------|
| `opencnpj-core` | CNPJ identifiers (numeric and alphanumeric check digits), RFB layouts, lossless field codecs, Latin-1 decoding, search normalization |
| `opencnpj-store` | Snapshot writer/reader, columnar blocks, compression, FST + postings index, query execution |
| `opencnpj-ingest` | ZIP streaming with limits, RFB CSV dialect parser, bucketed external sort, parallel block encoding, index merge |
| `opencnpj` | The binary: `download`, `update`, `rollback`, `build`, `verify`, `get`, `search`, `export`, `reconstruct`, `serve`, `bench`; the public API (`api.rs`, `params.rs`), the admin suite (`admin.rs`, `admin.html`) and request metrics (`metrics.rs`) |

## Build pipeline

```text
download ─► ZIPs (+ .sha256, source-manifest.json)
              │
 phase A      ▼  reader threads: inflate one ZIP member each, cut 4 MiB chunks at record
 partition       boundaries ──bounded channel (backpressure)──► parser workers
                 parse `";"`-separated records, keep rejects verbatim, count dictionary
                 values exactly, append zstd-1 frames to ~1000 key-range bucket files
                 (first 3 bytes of cnpj_basico ⇒ bucket order = key order)
              │
 phase B      ▼  per bucket, in parallel: load 4 tables, sort by key (+ branch, source, line),
 encode          merge-join into companies, encode columnar blocks, compress, emit
                 (term, doc) pairs → sorted run file
              │
 phase C      ▼  concatenate segments (global doc ids by prefix sum), k-way merge runs into
 assemble        FST + postings (+ município/CNAE doc values from the same term stream),
                 write dictionaries/manifest/section table/footer,
                 fsync, rename into place, write SHA-256 sidecar
```

Memory is bounded by the channel depth, per-worker buffers and one bucket per worker;
it does not grow with the dataset. Phase A output can be kept (`--keep-partition`) to
rebuild with other profiles without re-reading the ZIPs (used by the experiments).

## Decisions

### One file, company-clustered blocks

All rows of a `cnpj_basico` (empresa, establishments, partners, Simples) are stored
together, sorted by key. A CNPJ lookup is one binary search over an in-memory block
directory (40 B/block), one block read and one decompression; relationships need no join.
Alternatives considered: one file per table (needs joins and four lookups), a key → offset
hash/MPHF (more bytes than a sparse directory for no measurable gain when a block must
be decompressed anyway).

### Columnar inside the block, generic compressor on top

Each block stores one stream per column, then compresses the block as a whole with zstd
(optionally with a trained dictionary). Type-specific codecs (dates as day numbers, money
as cents, CEP as integers, CNAE lists as integers, low-cardinality columns as global
dictionary ids, delta-coded keys) remove redundancy that zstd cannot see across blocks;
anything that does not round-trip exactly is stored verbatim, so the encoding is
lossless by construction (property-tested on arbitrary strings).

### Inverted index: FST + adaptive postings

Terms (name tokens and filter values) live in one FST (`fst` crate); postings are either
a delta-varint list or a Roaring bitmap, whichever is smaller. Filters are bitmap
intersections; counts are exact. Tantivy was not used: it stores its own copy of the
documents or requires a separate store, adds segment management and is far larger than
what the queries need (token AND + prefix + bounded fuzzy + filter bitmaps). DuckDB/
Parquet and SQLite are measured as baselines instead.

Ranking is computed with bitmaps too: per-field "name length" terms let the engine take
the shortest matching names first without visiting every candidate.

`group_by` on município and CNAE principal reads a forward column (one bit-packed value
id per document, `docvalues` section) and counts the candidates in one pass: documents are
ordered by CNPJ, so intersecting thousands of value postings with a large candidate set
touched every Roaring container once per value. Secondary CNAE is multi-valued and keeps
the postings path (see `docs/BENCHMARKS.md` for the measured trade-off).

### Three access structures, each for what it does best

* **Postings** (FST + Roaring) answer equality and prefix filters and text tokens:
  `uf`, `cnae` (sections and prefixes expand to term ranges), `cep`, CPF and partner
  documents.
* **Doc values** (one dictionary id per document, format 1.1) answer `group_by` on
  município and CNAE in one pass over the candidates.
* **Numeric columns** (format 1.2, 1,024-document frame-of-reference blocks with min/max)
  answer date and capital ranges, "no value" conditions, sorting and yearly histograms.
  Opening dates follow the CNPJ order closely, so most blocks are skipped or taken whole
  without decoding.

CNPJ prefixes need none of them: documents are stored in CNPJ order, so a prefix is a
contiguous document range found from the block directory.

### Memory-mapped, immutable snapshots

The reader maps the file and validates header, footer, section table and section
checksums at open; block checksums are verified on every read. Snapshots are never
modified after publication (temporary file + rename), which is the invariant that makes
the mapping sound. Updates build a new file and switch a pointer file atomically; the API
swaps `Arc<Snapshot>` so in-flight queries finish on the old one, and `rollback` flips
back.

### Full rebuild per release

RFB publishes full snapshots, not deltas. A national build takes minutes (see
BENCHMARKS.md), so incremental maintenance (diffing 220 M rows, patching blocks and
postings) would add complexity to save little. `update` skips the build when every
source ZIP has the same SHA-256 as the current snapshot's manifest.

## Serving and operations

`serve` runs two listeners: the public API and the admin suite (dashboard + JSON API).
CPU-bound work runs on the blocking pool behind a semaphore with a deadline; a middleware
records every request into per-route log-scale latency histograms and a per-second ring
buffer (`metrics.rs`), exported to the dashboard and to Prometheus. Admin jobs (verify,
bench, download, build, update) run this binary as child processes with log files, so a
crash or a long build never takes the server down; switching snapshots swaps an
`Arc<Snapshot>` while in-flight requests finish on the old one.

Data routes pass through two middlewares (`gate.rs`). The gate resolves the credential
(a verified key is cached by its SHA-256, so Argon2 runs once per key per hour; session
tokens are SHA-256 lookups), checks the account status and plan, takes a token from the
caller's bucket and a unit from its daily and monthly counters (all in memory), and after
the response records usage into a map that a background task writes to SQLite every five
seconds in one transaction; search-log rows go through a bounded channel to a writer
thread. The response cache (moka, W-TinyLFU, weighted by bytes) sits inside the gate:
identical concurrent requests are coalesced into one computation, keys include the
snapshot identity, and `ETag`/`If-None-Match` saves the body entirely. CNPJ lookups and
heavy queries (searches, stats, CPF, export) run behind separate concurrency lanes.

On SIGTERM a process writes its verified keys and cached responses to the data directory
for its successor, answers with `Connection: close` for `--drain-secs`, then drains; with
`--reuse-port` both processes share the listening sockets, so an upgrade drops no request
(DEPLOYMENT.md). Accounts, roles, plans and moderation live in `accounts.rs`
(`docs/ACCOUNTS.md`), the admin API in `manage.rs`, release checks in `autoupdate.rs`.

## Profiles

All profiles store identical semantic content (the integration test asserts this). They
differ only in block size, codec and dictionary; the trade-off is measured in
BENCHMARKS.md.

## Not in scope (yet)

A public web portal, billing, signed releases, GPU paths, a custom memory allocator —
none showed a measured need.
