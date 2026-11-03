# Data, provenance and LGPD

## Source

Receita Federal do Brasil publishes the "Dados Abertos do CNPJ" monthly through a public
Nextcloud share (`https://arquivos.receitafederal.gov.br`, WebDAV at
`public.php/webdav`, folder `YYYY-MM/`). Each release has 37 ZIPs: `Empresas0-9`,
`Estabelecimentos0-9`, `Socios0-9`, `Simples` and six reference tables. Each ZIP holds one
`;`-separated, double-quoted, ISO-8859-1 CSV. Layouts follow the RFB "Metadados CNPJ"
document; see [legacy/DATA_FORMATS.md](legacy/DATA_FORMATS.md) and
`crates/opencnpj-core/src/schema.rs`.

OpenCNPJ does not claim any right over this data. Check RFB's current terms before
redistributing raw files or derived snapshots.

## What the snapshot preserves

| Guarantee | Provided by | How it is checked |
|-----------|-------------|-------------------|
| **Semantic**: every row, every field value (decoded to UTF-8), every link between tables | the `.ocnpj` snapshot | `opencnpj verify --sources DIR` compares per-table row multisets (count + 64-bit hash sum) between ZIPs and snapshot |
| **Byte-for-byte**: the original files | the original ZIPs, kept next to `source-manifest.json` (size, ETag, Last-Modified, SHA-256) | `sha256sum -c` / `opencnpj verify --sources` (size check) |

A CSV rewritten from a snapshot (`opencnpj reconstruct`) has the same values but is not
byte-identical to the RFB file: it is UTF-8, rows are ordered by CNPJ, and quoting is
regenerated. Keep the ZIPs when byte-level evidence matters (they compress poorly further:
UrnaPack's measurements found <1% extra gain over already-zipped downloads).

Lines that do not fit the layout are stored verbatim (Latin-1 bytes, file, line number,
reason) in the `rejects` section and counted in the manifest. The 2026-10 release had none.
Duplicate keys are kept, not deduplicated, and counted (`duplicate_empresa_keys`, …).

Search structures are derived: tokens are uppercased and accent-folded, stopwords
removed — the stored values are never altered.

## Chain of trust and its limits

1. The share publishes **no checksums or signatures**. TLS protects transport only.
2. `download` records sizes and computes SHA-256 **locally**; ZIP CRC-32 is verified while
   building. This proves the bytes did not change *after* download, not that they came
   from RFB.
3. The snapshot manifest embeds each source's SHA-256, and the snapshot's own SHA-256 is
   published alongside it (`/api/v1/snapshot`). Anyone with the same ZIPs and the same
   version can rebuild and compare.
4. A manifest is not an authenticity proof. Signing releases (e.g. minisign/Sigstore) is
   left to whoever distributes snapshots.

## Personal data (LGPD, Lei 13.709/2018)

The release contains personal data: names of partners (sócios) and legal representatives,
partially masked CPFs (`***123456**`), age brackets, and for sole proprietorships (MEI,
EI) the company name often *is* a person's name, plus addresses, phones and e-mails.
The data is made public by RFB under its legal duties, but LGPD still applies to anyone
who further processes it (purpose, necessity, transparency — art. 6, and art. 7 §§3–4 for
publicly accessible data).

Defaults and controls in this project:

* No full-text index over partner names or CPFs; you cannot search people by name.
* `serve --hide-socios` and `--hide-contacts` remove partners and phone/e-mail fields
  from every API response.
* Bulk export is **off** unless `--enable-export`, and capped by `--export-max-rows`.
* Ranked results cannot be paged deeper than `--max-offset` (default 1000); filter
  queries page by cursor, so full enumeration requires deliberate, observable traffic.
* Requests are bounded by `--max-concurrency` and `--timeout-ms`.
* There is no per-client rate limiting inside the binary. Public deployments should put a
  reverse proxy in front (e.g. nginx `limit_req`, or an API gateway with keys) and keep
  access logs only as long as needed.

Operators are the controllers of their deployment: document the purpose, honour data
subject requests that apply to them, and do not combine this data with other sources to
profile individuals.
