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

## Rules

- Subject max **72 characters**
- Use imperative mood (`add`, not `added`)
- Scope is optional but recommended (`parser`, `api`, `web`)

## Examples

- `feat(parser): add Latin-1 to UTF-8 conversion in CSV reader`
- `perf(loader): replace batch insert with COPY FROM STDIN`
- `test(parser): cover date null edge case`
- `docs(security): translate policy to English`

Enforced locally via `.commitlintrc.yml` and in CI when configured.
