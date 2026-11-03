# Security

## Reporting

Report vulnerabilities privately through GitHub Security Advisories
("Report a vulnerability" on the repository's Security tab). Do not open public issues
for security problems.

## Threat model

| Input | Trust | Controls |
|-------|-------|----------|
| RFB ZIPs | Untrusted bytes (no published checksums) | One regular member required, name validated, declared size ≤ 64 GiB and ≤ 200× the archive, reads stop past the declared size, CRC-32 checked; nothing is extracted to disk. Malformed lines are kept as rejects, never executed or interpreted. |
| `.ocnpj` snapshots | Untrusted unless you built them | Header/footer magic, version, file length, section table and every section checksummed (xxh3); all offsets bounds-checked; block decompression bounded (`MAX_BLOCK_RAW`, exact size); postings ids range-checked; random-corruption test asserts errors instead of panics. Checksums detect corruption, not forgery — verify the SHA-256 against a trusted source. |
| HTTP requests | Untrusted | Path/query length limits, page size and offset caps, bounded fuzzy/prefix expansion, CPU work behind a semaphore, per-request timeout, export opt-in, no write endpoints. |
| `data/current` pointer | Operator-controlled | Must name a file inside the data dir (`..` rejected). |

`unsafe` code: one `Mmap::map` (published snapshots are immutable by construction;
documented at the call site) and one `posix_fadvise` call in the benchmark harness.

## Tooling

```bash
cargo audit                     # RustSec advisories (clean at the time of writing)
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # includes corruption/truncation tests
```

Run behind a reverse proxy with TLS and rate limiting for public deployments; see
[DATA_POLICY.md](DATA_POLICY.md) for personal-data controls.
