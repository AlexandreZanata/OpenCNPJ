# opencnpj-insights

This library houses catalogue loaders, aggregate readers and writers, query
planning, execution backends and answer rendering. It uses the existing
snapshot reader and search engine through `opencnpj-store`.

See the [data asset design](../../docs/intelligence/09-DATA-ASSETS.md),
[query planning design](../../docs/intelligence/04-QUERY-IR.md#7-planning-and-execution)
and [response design](../../docs/intelligence/08-RESPONSES.md).

## Municipality catalogue

`catalog::municipalities::MunicipalityCatalogue::load` reads the generated
`municipalities.tsv` and checks the schema, identifiers and neighbour graph.
Rows are assigned dense IDs in TOM order and can also be looked up by TOM or
IBGE code. The catalogue lives under `/data/opencnpj-data/intelligence/`.

`pop_census_2022` is empty for Boa Esperança do Norte, whose municipality was
installed in 2025. `pop_est` and `pop_est_year` carry the official IBGE
estimate and reference year. The loader exposes the census field as `None`.
The `9707` row represents establishments abroad and has no municipal
geography or population.

The neighbour graph retains links between municipalities and the districts
represented as municipalities by IBGE. The two state operational areas in
the boundary workbook, Lagoa Mirim and Lagoa dos Patos, are outside this graph.
