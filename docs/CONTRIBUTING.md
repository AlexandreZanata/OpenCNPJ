# Contributing

## Local setup

```bash
cp .env.example .env
docker compose up -d postgres redis
go run ./cmd/migrate
make test
```

## Workflow

1. Create a feature branch.
2. Follow [COMMIT_CONVENTION.md](COMMIT_CONVENTION.md) (`type(scope): subject`, max 72 chars).
3. Run `make test`, `make vet`, and `make lint` before opening a PR.
4. Add unit tests for changed code; register new features in [DVT.md](DVT.md).

## Fixtures

```bash
make seed   # loads tests/fixtures (≤ 10k rows per file)
```

## Security checks

```bash
./scripts/security-check.sh
```

See [SECURITY-COMMANDS.md](SECURITY-COMMANDS.md) for tool details.

## AI-assisted changes

Read [AGENTS.md](../AGENTS.md) and `.cursor/rules/` before using Cursor agents on this repo.
