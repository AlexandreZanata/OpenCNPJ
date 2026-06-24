# Commit Convention

Format:

`type(scope): subject`

## Allowed types

- feat
- fix
- perf
- refactor
- test
- bench
- docs
- ci
- chore
- build
- revert

## Allowed scopes (CI `scope-enum` — mandatory)

| Scope | Use for |
|-------|---------|
| `parser`, `loader`, `pipeline`, `importer` | Import pipeline |
| `downloader` | RFB download bot |
| `model`, `repository`, `handlers`, `services`, `middleware` | Go packages |
| `api`, `web`, `export` | HTTP API / frontend |
| `config`, `metrics` | Configuration / observability |
| `migration`, `fixtures` | SQL migrations / test data |
| `docs`, `oss`, `security` | Documentation |
| `ci`, `workflow` | GitHub Actions |
| `git` | `.gitignore`, git hooks, repo hygiene |
| `docker` | `docker-compose.yml`, Dockerfiles |
| `scripts` | `scripts/` shell tooling |
| `deps` | `go.mod`, `package.json` dependencies |
| `bench`, `benchmarks` | Benchmarks (`docs/benchmarks/`, TSV results) |

Full list (must match `commitlint.config.mjs`):

`parser`, `loader`, `pipeline`, `model`, `config`, `metrics`, `ci`, `docs`, `bench`, `benchmarks`,
`migration`, `fixtures`, `importer`, `downloader`, `oss`, `api`, `web`, `export`, `security`,
`git`, `docker`, `scripts`, `workflow`, `deps`, `repository`, `handlers`, `services`, `middleware`, `commitlint`

## Rules

- Subject max **72 characters**
- Use imperative mood (`add`, not `added`)
- Scope must be from the table above — **CI fails on unknown scopes**

## Examples

- `feat(parser): add Latin-1 to UTF-8 conversion in CSV reader`
- `perf(loader): replace batch insert with COPY FROM STDIN`
- `chore(git): ignore node_modules and scan artifacts`
- `ci(workflow): fix commitlint scope list`
- `docs(security): translate policy to English`

Enforced in CI via `commitlint.config.mjs` and `.commitlintrc.yml`.
