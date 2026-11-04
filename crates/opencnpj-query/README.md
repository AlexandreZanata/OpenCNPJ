# opencnpj-query

This library defines the contract between question interpretation and execution.
Its modules cover typed requests, canonicalization, versioned encoding, cache
keys, validation and textual rendering.

The [query IR design](../../docs/intelligence/04-QUERY-IR.md) defines the contract.
The library keeps I/O and async execution outside that contract.
