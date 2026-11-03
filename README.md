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
$B serve --snapshot data/2026-10.ocnpj --bind 127.0.0.1:8080
curl 'localhost:8080/api/v1/cnpj/00000000000191'
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
* **Search**: accent-insensitive multi-word name search with prefix and bounded fuzzy
  matching, ranking, and filters by UF, município, CNAE (principal/secundária),
  situação, porte, natureza jurídica, Simples, MEI and matriz/filial — exact counts.
* **Integrity**: checksums on every block and section, SHA-256 of sources and snapshot,
  strict bounds checks; corrupted or truncated files are rejected.
* **Operations**: one static binary for Linux and Windows, atomic publication, hot
  reload, timeouts, concurrency limits, Prometheus metrics, privacy switches.

Measured numbers (build time, size, latency, comparison with ZIP/CSV/Parquet/SQLite and
the legacy PostgreSQL stack) are in [docs/BENCHMARKS.md](docs/BENCHMARKS.md), with the
commands to reproduce them.

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
