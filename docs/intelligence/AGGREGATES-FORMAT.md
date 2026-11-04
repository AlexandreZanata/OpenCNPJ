# `.ocagg` format v0.1

An `.ocagg` is a derived, immutable artefact for one verified OpenCNPJ
snapshot and one municipality catalogue. The embedded manifest identifies
both inputs by SHA-256. The CLI writes an adjacent `.sha256` file for
distribution. Integers in the header, directory and footer are little endian.

## Container

The file begins with this 32-byte header, followed by the UTF-8 JSON
manifest, block payloads, block directory and footer in that order.

| Offset | Bytes | Value |
|---|---:|---|
| 0 | 8 | `OCAGG` followed by three zero bytes |
| 8 | 2 | Major version `0` |
| 10 | 2 | Minor version `1` |
| 12 | 4 | Manifest length |
| 16 | 4 | Number of block entries |
| 20 | 8 | Absolute offset of the block directory |
| 28 | 4 | Reserved, zero |

The manifest records the snapshot competence as months since `2000-01`,
the two input SHA-256 strings, the sorted selected tables, the establishment
count and missing-date counters for the selected flow tables. Older history
artefacts can contain only T1 and T2.

Each 48-byte directory entry has a section tag (`u8`), codec (`u8`), cell
count (`u16`), raw and stored lengths (`u32` each), four reserved zero bytes,
the absolute payload offset (`u64`), first and last keys (`u64` each) and
an xxh3-64 checksum of the raw block (`u64`). Entries are in section and key
order. Payloads are contiguous. Codec `0` stores the bytes as written and
codec `1` stores a Zstandard frame. The writer uses compression when it
reduces the payload size.

A block has up to 4,096 cells. The raw bytes contain a one-byte count bit
width followed by one unsigned LEB128 key delta per cell. The first delta
is from zero. The remaining deltas are positive. The counts follow in key
order, packed least-significant bit first at the block width. Final padding
bits are zero. Counts are positive. The last 16 bytes contain the xxh3-64
checksum of all preceding bytes and the eight-byte marker `OCAGEND!`.

The reader maps the file read-only, validates the directory bounds and the
file checksum, and checks payload size, checksum and ordering on decode.
Point lookups use the directory. Keep mapped artefacts immutable and publish
new versions as separate files.

## Sections and keys

Section tags are `(family << 3) | table`, with table IDs T1 through T5
represented by integers `1` through `5`.

| Family | Meaning | Key and value |
|---:|---|---|
| 0 | Table cells | Packed cell key and document count |
| 1 | CNAE marginals | CNAE code and document count |
| 2 | UF marginals | UF ID and document count |
| 3 | Municipality marginals | Dense municipality ID and document count |
| 4 | UF × CNAE marginals | `(UF << 24) | CNAE` and document count |
| 5 | Per-CNAE block index | CNAE code and one-based first block ordinal within its cell section |

A cell key is `(CNAE << 38) | (municipality << 22) | (UF << 17) | detail`.
CNAE uses the exact raw seven-digit subclass and has value zero when absent.
Municipality uses the dense catalogue ID with `65535` reserved for missing.
UF uses the snapshot's indexed UF, so an establishment whose municipality's
official UF differs still contributes to the same UF as the engine. UF IDs
are alphabetic by abbreviation with EX appended and `31` for missing.

| Table | Grain | Detail |
|---|---|---|
| T1 | Active stock by main CNAE and place | Head office flag in bits 0–1, porte in bits 2–4, MEI in bits 5–6 and Simples in bits 7–8 |
| T2 | All stock by main CNAE and place | Head office flag in bits 0–1 and status in bits 2–4 |
| T3 | Registrations by main CNAE, place and period | Flow bucket based on cadastral activity start date |
| T4 | Closures by main CNAE, place and period | Flow bucket based on status date for situação `08` |
| T5 | Active presence by secondary CNAE and place | Zero |

Head office and branch use `1` and `2`. Porte uses `1..=4` for `00`, `01`,
`03` and `05`. MEI and Simples use `1` for `N` and `2` for `S`. Status uses
`1..=5` for `01`, `02`, `03`, `04` and `08`. Zero in the categorical
columns represents a missing value. T5 counts a document once for each
distinct secondary subclass. A sum across secondary codes may count the
same establishment more than once. Families 1–4 are stored for T1–T4.

Flow buckets are one plus the number of months from January 1900. Dates
before 2021 use their year's January bucket for annual granularity. Dates
from 2021 use monthly granularity. A missing date stays out of the flow
tables and increments the corresponding manifest counter. Company documents
with no establishment stay out of the aggregate tables.
