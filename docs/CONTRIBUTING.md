# Contributing

## Ambiente local

- Suba dependencias com `docker compose up -d`.
- Configure `DATABASE_URL`.
- Execute `make migrate` para preparar schema.

## Fluxo recomendado

- Crie branch de feature.
- Use commits convencionais.
- Rode `make lint test bench`.
- Abra PR com CI verde.

## Fixtures

- Gere fixtures com `make seed`.
- Fixtures devem ter no maximo 10k linhas por arquivo.
