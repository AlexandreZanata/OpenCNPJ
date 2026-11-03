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
| `core` | Identifiers, schema, codecs (`opencnpj-core`) |
| `store`, `format`, `search` | Snapshot format, reader/writer, query engine (`opencnpj-store`) |
| `ingest`, `parser`, `downloader` | Download, ZIP/CSV parsing, build pipeline (`opencnpj-ingest`) |
| `api`, `cli`, `bench` | Binary crate (`opencnpj`) |
| `docs`, `ci`, `deps` | Documentation, workflows, dependencies |

## Rules

- Subject max **72 characters**
- Use imperative mood (`add`, not `added`)
- Prefer a scope from the table above; omit it for repo-wide changes

## Examples

- `feat(search): rank prefix expansions by term frequency`
- `perf(store): decode only the requested establishment row`
- `fix(parser): keep lines with embedded line breaks`
- `docs(format): document postings encoding`
