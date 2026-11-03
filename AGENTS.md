# AGENTS.md — OpenCNPJ

Rust engine for Receita Federal CNPJ open data: download, `.ocnpj` snapshot build,
search, HTTP API. English only (code, comments, docs, commits).

| Path | Purpose |
|------|---------|
| `crates/opencnpj-core` | CNPJ ids, RFB layouts, lossless codecs, normalization |
| `crates/opencnpj-store` | Snapshot format, reader/writer, index, query engine |
| `crates/opencnpj-ingest` | ZIP/CSV streaming, build pipeline |
| `crates/opencnpj` | CLI + HTTP API + benchmark harness |
| `docs/` | ARCHITECTURE, FORMAT, API, DATA_POLICY, BENCHMARKS |
| `docs/legacy/` | Archived Go implementation notes and historical benchmarks |
| `scripts/` | Experiment matrix and baselines (Parquet, SQLite) |

## Rules

- Never trade correctness for speed: every source row must round-trip
  (`opencnpj verify --sources`). Do not drop rows silently.
- Snapshot layout changes update `docs/FORMAT.md` and the format version.
- Do not report a performance number without the command and raw output.
- No PostgreSQL/Redis/Elasticsearch or paid services.
- No national datasets or large binaries in Git.

## Delivery gate

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Commits: `type(scope): subject` (≤ 72 chars), see `docs/COMMIT_CONVENTION.md`.
Do not commit or push unless asked. The legacy Go code is at tag `legacy-go-final`.
