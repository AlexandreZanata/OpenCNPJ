# opencnpj-query

This library defines the contract between question interpretation and execution.
Its modules cover typed requests, canonicalization, versioned encoding, cache
keys, validation and textual rendering.

The [query IR design](../../docs/intelligence/04-QUERY-IR.md) defines the contract.
The library keeps I/O and async execution outside that contract.

`Request::canonicalize` makes status defaults explicit, expands sector
filters, clips date ranges to the data range and reconciles geographic
filters using the supplied catalogue context. It returns presentation order
separately so a reordered comparison can share a cache entry.

`validate::validate(request, &capabilities)` returns a `ValidRequest` or the
issues found (`Missing`, `Ambiguous`, `Impossible`) with complete IR options
for the clarification. `ValidRequest` has a private constructor, so execution
can only receive requests that passed validation.

`codec::encode` and `decode` use the version byte, stable enum tags and
unsigned LEB128 integers. Set encodings must be sorted and unique. Catalogue
versions and the aggregate identity invalidate cache keys when data changes.
The IR also supports serde JSON and a textual `Display` implementation.

The `tests/fixtures/ir-v1-*.jsonl` files pin input examples, canonical bytes
and textual output. Days follow the snapshot numeric column convention,
where `1` represents `1900-01-01`. Density metrics encode their stock basis
implicitly in v1 and carry an explicit IBGE population reference.
