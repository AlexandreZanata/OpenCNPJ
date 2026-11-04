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

What the engine indexes and exposes (format 1.2), and the controls available:

* **CPF search is enabled** (requested by the project owner). Individual entrepreneurs
  and MEIs have the owner's full CPF embedded in the company name ("MARIA DA SILVA
  12345678901") — 17.3 million such names in 2026-10. Those 11-digit numbers are indexed
  separately (`cpf_titular`), so a CPF finds the person's businesses.
* Partner and legal-representative CPFs are published **masked** (`***123456**`). The
  engine indexes only the 6 visible digits; a CPF search over partners is therefore a
  *possible* match (others can share those digits), and `/api/v1/cpf` returns the partner
  rows so the name can be checked. The engine never reconstructs a masked CPF.
* Partner names are not indexed for full-text search.
* `serve --hide-socios` and `--hide-contacts` remove partners and phone/e-mail fields
  from every API response.
* **Accounts and limits are on by default** (`--auth required`): every data request is
  tied to an account and metered by its plan (rate limit, daily/monthly quotas, page size,
  export and download rights). Staff can suspend or ban accounts, and every staff action is
  in the audit log. `--auth off` removes all of this; use it only on a private network.
* Restrict `/api/v1/cpf` and the `cpf` filters (or use `--hide-socios`) if the
  deployment's purpose does not require locating people's businesses by CPF.

### Data about API users

The accounts database (`accounts.sqlite`) holds, per user: username, e-mail address, role,
plan, status and moderation notes; Argon2id hashes of login keys (never the keys);
SHA-256 hashes of session and recovery tokens; per-day request counts by type; session
IP address and user agent; and, while the `search_log` setting is on, each data request
(time, route, query string, status, latency, client IP) for `search_log_days` days
(default 30), after which a background task deletes it. The audit log keeps staff
actions with the actor's address. Deleting an account removes its keys, sessions,
recovery tokens, usage rows and search log; audit entries keep the username that was
acted on.

Query strings can contain CPFs and names typed by users: keep the retention short, turn
`search_log` off when it is not needed, and restrict `usage.read` to staff who need it.
Use `opencnpj accounts backup` for consistent copies and protect them like the live file.

Operators are the controllers of their deployment: document the purpose, honour data
subject requests that apply to them, and do not combine this data with other sources to
profile individuals.
