# `.ocnpj` snapshot format, version 1.0

One immutable file per competence (monthly release) and build profile. Everything
needed to answer queries — data, dictionaries, block directory, term dictionary and
postings — lives in that file. No external database, no sidecar index.

All integers are little-endian. Readers must validate every offset and length against
the file size before using it; the reference reader (`opencnpj-store::reader`) does.

```text
┌──────────────┬──────────────────────────────┬──────────────────┬───────────────┬────────────┐
│ Header 64 B  │ DATA: compressed blocks …    │ other sections … │ section table │ Footer 40 B│
└──────────────┴──────────────────────────────┴──────────────────┴───────────────┴────────────┘
```

## Header (64 bytes)

| Offset | Size | Field |
|-------:|-----:|-------|
| 0 | 8 | magic `OCNPJ\0v1` |
| 8 | 2 | format major (`1`); readers reject other majors |
| 10 | 2 | format minor (`0`); additive changes only |
| 12 | 4 | flags (reserved, `0`) |
| 16 | 8 | competence, ASCII `YYYY-MM`, NUL-padded |
| 24 | 32 | reserved (zero) |
| 56 | 8 | xxh3-64 of bytes 0..56 |

## Footer (40 bytes, at end of file)

`table_offset u64, table_len u64, table_xxh3 u64, file_len u64, magic "OCNPJEND"`.
`file_len` must equal the actual file size, so truncation is detected at open.

## Section table

`count u32` (≤ 64) followed by `count` entries of 40 bytes:
`kind u32, codec u32 (0 raw, 1 zstd), offset u64, len u64, raw_len u64, xxh3 u64`.
Each kind appears at most once. `xxh3` covers the stored (possibly compressed) bytes.

| Kind | Name | Content |
|-----:|------|---------|
| 1 | `data` | Concatenated compressed blocks (checksum covers the whole region) |
| 2 | `manifest` | JSON: schema, provenance (source files with SHA-256), counts, build settings |
| 3 | `blockdir` | Block directory, 40 B per block (below) |
| 4 | `dictionaries` | Global column dictionaries (zstd) |
| 5 | `zstddict` | Trained zstd dictionary shared by all blocks (optional) |
| 6 | `reftables` | Reference tables (CNAE, municípios, …) as JSON, source order (zstd) |
| 7 | `rejects` | Source lines that did not fit the layout, verbatim Latin-1 bytes (zstd) |
| 8 | `terms` | FST (`fst` crate format) mapping term → postings offset |
| 9 | `postings` | Postings lists |

Unknown kinds are an error in 1.0. Future minors may add kinds that 1.0 readers ignore
only if the major version is unchanged and the manifest says so.

## Data model and blocks

The unit of storage is a **company**: one `cnpj_basico` with all of its rows from the
four fact tables (`empresas`, `estabelecimentos`, `socios`, `simples`). Companies are
sorted by `cnpj_basico` (byte order, which is also the order of the alphanumeric CNPJ),
so everything about a company is physically contiguous and a lookup touches one block.

A block holds whole consecutive companies until its uncompressed size reaches the
profile's target. Raw block layout:

```text
varint n_companies
varint n_streams                (= 5 + total row columns = 61)
n_streams × varint stream_len
stream 0: keys        numeric keys delta-coded within the block; others verbatim
stream 1..4: row counts per company for empresas, estabelecimentos, socios, simples
stream 5..: one stream per row column, tables in that order, columns in layout order
```

Each block is compressed independently (zstd with an optional shared trained
dictionary, or LZ4, or none). Block directory entry (40 B):
`first_key [8]u8, offset u64, clen u32, rlen u32, first_doc u32, companies u32, xxh3 u64`
(`xxh3` of the compressed bytes, checked on every read).

### Field codecs (lossless)

Every value is a string in the semantic model (exactly the decoded source field).
Typed codecs are used only when formatting the typed value reproduces the source string
byte for byte; otherwise the value is escaped (stored verbatim).

| Kind | Typed form | Examples kept verbatim |
|------|-----------|------------------------|
| `text` | length-prefixed UTF-8 | — |
| `dict` | global dictionary id (values seen ≥ 2 times) | rare values |
| `date` | days since 1000-01-01 (+1), `00000000` → 0 | `20230229`, `2007040` |
| `money` | cents (`1000,00` → 100000) | `1.000,00`, `01,00`, `1,5` |
| `digits(n)` | integer, zero-padded to `n` on decode | `0131010A` |
| `cnae_list` | count + 7-digit codes | `7020400,` |

Tagged kinds start with a varint: `0` empty string, `1` escaped string follows,
`n ≥ 2` typed value `n - 2`.

## Search documents and index

A **document** is one establishment, or one company that has no establishment rows.
Documents are numbered in block order (so ids are sorted by CNPJ) and `first_doc` in the
block directory maps a document to its block.

Terms are `[field u8][value]`:

| Field | Values |
|------:|--------|
| 1 `razao_social` | normalized tokens (uppercase ASCII, accents folded, stopwords `DA DAS DE DI DO DOS E` removed), propagated to every establishment of the company |
| 2 `nome_fantasia` | normalized tokens |
| 3 `razao_len`, 14 `fantasia_len` | token count bucket 0..15 (ranking) |
| 4 `uf`, 5 `municipio`, 6 `cnae`, 7 `cnae_secundaria`, 8 `situacao`, 13 `matriz_filial` | establishment values |
| 9 `porte`, 10 `natureza`, 11 `simples`, 12 `mei` | company values, propagated to its establishments |

Postings entry: `varint len, tag u8, payload` — tag 0 is a delta-varint list
(`count`, first id, gaps), tag 1 a Roaring bitmap in the portable serialization. The
builder writes whichever is smaller.

## Integrity

* Header, section table, every section and every block carry xxh3-64 checksums.
* The file's SHA-256 is written next to it (`<file>.ocnpj.sha256`) and reported by the
  API; it cannot live inside the file it hashes.
* `opencnpj verify --sources DIR` recomputes per-table multiset fingerprints of the source
  CSV rows and of the rows decoded from the snapshot and requires them to match.

## Compatibility

* Major version change: incompatible layout. Old readers refuse the file; rebuild from
  the source ZIPs (snapshots are derived artifacts, the ZIPs are the source of truth).
* Minor version change: new optional sections or manifest fields only.
