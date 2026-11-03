# OpenCNPJ

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](docs/LICENSING.md)

A compact, offline search engine for the Brazilian CNPJ open data published monthly by
Receita Federal. One Rust binary downloads the release, turns its 37 ZIPs (~29 GB of
CSV, ~224 million rows) into a single compressed, indexed `.ocnpj` file, verifies it
against the sources, and serves CNPJ lookups, name search, filters, statistics and
exports over HTTP. No PostgreSQL, Redis or search cluster.

> The previous Go + PostgreSQL + Redis + React implementation is archived at tag
> [`legacy-go-final`](docs/legacy/README.md).

## Quick start

```bash
cargo build --release
B=target/release/opencnpj

$B download --dest data/raw                                  # latest complete release
$B build --input data/raw/2026-10 --output data/2026-10.ocnpj
$B verify data/2026-10.ocnpj --sources data/raw/2026-10      # row-for-row check
$B get data/2026-10.ocnpj 33.000.167/0001-01
$B search data/2026-10.ocnpj --q "padaria sao jo" --filter uf=SP --limit 5
$B serve --snapshot data/2026-10.ocnpj        # API :8080, admin http://127.0.0.1:8081/admin
curl 'localhost:8080/api/v1/cnpj/00000000000191'
curl 'localhost:8080/api/v1/search?cnae=G&uf=SP&data_inicio_atividade_de=2026-01-01&ordenar=data_inicio_atividade:desc'
```

Monthly operation (download → rebuild only if sources changed → verify → atomic switch,
previous snapshot kept for rollback):

```bash
$B update --data-dir data --profile balanced
$B serve --data-dir data          # picks up data/current automatically
$B rollback --data-dir data
```

## What you get

* **Lossless semantic storage.** Every row and value of empresas, estabelecimentos,
  sócios and Simples/MEI is recoverable exactly; lines that do not fit the layout are
  kept verbatim. `verify --sources` proves it per table against the ZIPs.
* **Numeric and alphanumeric CNPJ** (2026 format) with check-digit validation.
* **Search**: accent-insensitive multi-word name search with prefix matching, typo
  tolerance (edit distance + swapped letters, automatic retry) and ranking; CNPJ and CNPJ
  prefix, CPF (MEI/EI owners exactly, partners by their masked digits), partner CNPJ;
  filters by UF, município, CNAE (principal/secundária/either, by section, division,
  group or class), situação, motivo, porte, natureza, Simples/MEI (and *active*),
  matriz/filial, CEP prefix, DDD, published e-mail/phone; date and capital ranges
  (opening, status, Simples/MEI opt-in and exclusion); sorting by date or capital;
  exact counts, statistics per field or per year, streaming export.
* **API + admin suite**: no-auth, no-limit HTTP API with snapshot distribution
  (resumable downloads); an admin dashboard with live latency histograms, query explain
  with per-stage timings, snapshot switching, cache control, self-test benchmarks and
  background jobs (download, build, verify, update, benchmark).
* **Integrity**: checksums on every block and section, SHA-256 of sources and snapshot,
  strict bounds checks; corrupted or truncated files are rejected.
* **Operations**: one static binary for Linux and Windows, atomic publication, hot
  reload, timeouts, concurrency limits, Prometheus metrics, privacy switches.

## Measured on the 2026-10 national release

223,628,880 rows from 7.81 GB of ZIPs (28.86 GB of CSV), i7-13620H laptop, NVMe. Every
snapshot below passed `verify --sources` (all rows reproduced exactly).

| profile | snapshot (all indexes) | build | CNPJ lookup p50 / p95 (warm, 1 thread) | name search p95 |
|---|---:|---:|---:|---:|
| fast | 7.63 GB | ~5 min | 0.033 / 0.050 ms | 36–69 ms |
| balanced | 6.39 GB | 9 min 6 s | 0.158 / 0.189 ms | 40–83 ms |
| compact | 5.75 GB | ~11.7 min | 1.50 / 1.92 ms | 61–100 ms |

Through HTTP, the balanced snapshot served 32.7k CNPJ lookups/s at p95 0.8 ms
(16 connections, loopback). The legacy stack imported 217 M rows into PostgreSQL in
27 min 9 s. Methodology, the experiment matrix, baselines and known bottlenecks are in
[docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Documentation

| Doc | Content |
|-----|---------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Pipeline, crates, design decisions |
| [docs/FORMAT.md](docs/FORMAT.md) | `.ocnpj` byte layout, codecs, index, compatibility |
| [docs/API.md](docs/API.md) | HTTP endpoints, parameters, limits, legacy migration |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Methodology, results, experiments, Pareto front |
| [docs/DATA_POLICY.md](docs/DATA_POLICY.md) | Provenance, chain of trust, LGPD |
| [docs/legacy/](docs/legacy/README.md) | Archived implementation and how to restore it |
| [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) | Development workflow |

## License

MIT OR Apache-2.0, at your option. See [LICENSE](LICENSE), [LICENSE-APACHE-2.0](LICENSE-APACHE-2.0)
and [NOTICE](NOTICE). The CNPJ data belongs to Receita Federal do Brasil.
