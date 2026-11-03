//! JSON shaping, export, reconstruction and source comparison.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use opencnpj_core::schema::col;
use opencnpj_core::{Cnpj, Company, Row, Table, model::fixed_label};
use opencnpj_store::search::{self, Limits, Query, SearchResult};
use opencnpj_store::{DocRecord, Snapshot};
use rayon::prelude::*;
use serde_json::{Map, Value, json};
use xxhash_rust::xxh3::Xxh3;

/// What the API may expose. Everything is public RFB data, but operators can reduce
/// exposure of personal data (LGPD data-minimization).
#[derive(Debug, Clone, Copy, Default)]
pub struct Privacy {
    pub hide_socios: bool,
    pub hide_contacts: bool,
}

const CONTACT_COLUMNS: &[&str] = &["ddd_1", "telefone_1", "ddd_2", "telefone_2", "ddd_fax", "fax", "correio_eletronico"];

fn ref_table_for(column: &str) -> Option<&'static str> {
    Some(match column {
        "natureza_juridica" => "naturezas",
        "qualificacao_responsavel" | "qualificacao_socio" | "qualificacao_representante_legal" => "qualificacoes",
        "cnae_fiscal_principal" => "cnaes",
        "municipio" => "municipios",
        "pais" => "paises",
        "motivo_situacao_cadastral" => "motivos",
        _ => return None,
    })
}

pub fn row_json(snap: &Snapshot, table: Table, row: &Row, privacy: Privacy) -> Value {
    let mut m = Map::new();
    for (c, v) in table.row_columns().iter().zip(row) {
        if privacy.hide_contacts && CONTACT_COLUMNS.contains(&c.name) {
            continue;
        }
        m.insert(c.name.into(), Value::String(v.clone()));
        if v.is_empty() {
            continue;
        }
        let label = ref_table_for(c.name).and_then(|t| snap.label(t, v)).or_else(|| fixed_label(c.name, v));
        if let Some(l) = label {
            m.insert(format!("{}_descricao", c.name), Value::String(l.into()));
        }
    }
    Value::Object(m)
}

fn company_parts(snap: &Snapshot, c: &Company, privacy: Privacy) -> (Value, Value, Value) {
    let empresa = match c.empresas.as_slice() {
        [] => Value::Null,
        [e] => row_json(snap, Table::Empresas, e, privacy),
        many => Value::Array(many.iter().map(|e| row_json(snap, Table::Empresas, e, privacy)).collect()),
    };
    let socios = if privacy.hide_socios {
        Value::Null
    } else {
        Value::Array(c.socios.iter().map(|s| row_json(snap, Table::Socios, s, privacy)).collect())
    };
    let simples = match c.simples.as_slice() {
        [] => Value::Null,
        [s] => row_json(snap, Table::Simples, s, privacy),
        many => Value::Array(many.iter().map(|s| row_json(snap, Table::Simples, s, privacy)).collect()),
    };
    (empresa, socios, simples)
}

fn estab_cnpj(basico: &str, e: &Row) -> String {
    format!("{basico}{}{}", e[col::est::ORDEM], e[col::est::DV])
}

pub fn snapshot_ref(snap: &Snapshot) -> Value {
    json!({ "competence": snap.manifest.competence, "source_snapshot": snap.manifest.source_snapshot })
}

/// Lookup by 14-char CNPJ or 8-char root.
pub fn lookup(snap: &Snapshot, id: &str) -> Result<Option<Value>> {
    lookup_with(snap, id, Privacy::default())
}

#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    #[error("invalid cnpj: {0}")]
    Invalid(String),
}

pub fn lookup_with(snap: &Snapshot, id: &str, privacy: Privacy) -> Result<Option<Value>> {
    lookup_page(snap, id, privacy, 0, DEFAULT_ESTAB_PAGE)
}

/// Establishments returned per root lookup unless asked otherwise.
pub const DEFAULT_ESTAB_PAGE: usize = 100;

/// Like [`lookup_with`]; root lookups return establishments `offset..offset + limit`.
pub fn lookup_page(snap: &Snapshot, id: &str, privacy: Privacy, offset: usize, limit: usize) -> Result<Option<Value>> {
    let compact: String = id.chars().filter(|c| !matches!(c, '.' | '/' | '-' | ' ')).collect();
    if compact.len() == 8 {
        let basico = opencnpj_core::cnpj::parse_basico(&compact).ok_or_else(|| LookupError::Invalid("bad cnpj_basico".into()))?;
        let basico = std::str::from_utf8(&basico).expect("ascii");
        let Some((c, total)) = snap.company_page(basico, offset, limit)? else { return Ok(None) };
        let (empresa, socios, simples) = company_parts(snap, &c, privacy);
        let estabs: Vec<Value> = c
            .estabelecimentos
            .iter()
            .map(|e| {
                let mut v = row_json(snap, Table::Estabelecimentos, e, privacy);
                v.as_object_mut().expect("object").insert("cnpj".into(), Value::String(estab_cnpj(&c.basico, e)));
                v
            })
            .collect();
        return Ok(Some(json!({
            "cnpj_basico": c.basico,
            "empresa": empresa,
            "estabelecimentos": estabs,
            "total_estabelecimentos": total,
            "estabelecimentos_offset": offset,
            "socios": socios,
            "simples": simples,
            "snapshot": snapshot_ref(snap),
        })));
    }
    let cnpj = Cnpj::parse(&compact).map_err(|e| LookupError::Invalid(e.to_string()))?;
    let Some((c, total)) = snap.establishment(cnpj.basico(), cnpj.ordem(), cnpj.dv())? else { return Ok(None) };
    let e = &c.estabelecimentos[0];
    let (empresa, socios, simples) = company_parts(snap, &c, privacy);
    Ok(Some(json!({
        "cnpj": cnpj.as_str(),
        "cnpj_formatado": cnpj.formatted(),
        "cnpj_basico": c.basico,
        "estabelecimento": row_json(snap, Table::Estabelecimentos, e, privacy),
        "empresa": empresa,
        "socios": socios,
        "simples": simples,
        "total_estabelecimentos": total,
        "snapshot": snapshot_ref(snap),
    })))
}

/// Compact summary of one search document.
pub fn doc_summary(snap: &Snapshot, d: &DocRecord) -> Value {
    let c = &d.company;
    let razao = c.empresas.first().map_or("", |e| e[col::emp::RAZAO_SOCIAL].as_str());
    let mut v = json!({ "cnpj_basico": c.basico, "razao_social": razao });
    let m = v.as_object_mut().expect("object");
    if let Some(e) = d.estab.map(|i| &c.estabelecimentos[i]) {
        let get = |i: usize| Value::String(e[i].clone());
        m.insert("cnpj".into(), Value::String(estab_cnpj(&c.basico, e)));
        m.insert("nome_fantasia".into(), get(col::est::NOME_FANTASIA));
        m.insert("matriz_filial".into(), get(col::est::MATRIZ_FILIAL));
        m.insert("situacao_cadastral".into(), get(col::est::SITUACAO));
        if let Some(l) = fixed_label("situacao_cadastral", &e[col::est::SITUACAO]) {
            m.insert("situacao_cadastral_descricao".into(), Value::String(l.into()));
        }
        m.insert("uf".into(), get(col::est::UF));
        m.insert("municipio".into(), get(col::est::MUNICIPIO));
        if let Some(l) = snap.label("municipios", &e[col::est::MUNICIPIO]) {
            m.insert("municipio_descricao".into(), Value::String(l.into()));
        }
        m.insert("cnae_fiscal_principal".into(), get(col::est::CNAE_PRINCIPAL));
    }
    v
}

/// Search response; docs must come from `snap.docs(sorted ids)`.
pub fn search_json(snap: &Snapshot, r: &SearchResult, docs: Vec<DocRecord>, _full: bool) -> Value {
    let by_id: BTreeMap<u32, DocRecord> = docs.into_iter().map(|d| (d.doc, d)).collect();
    let results: Vec<Value> = r.docs.iter().filter_map(|id| by_id.get(id)).map(|d| doc_summary(snap, d)).collect();
    json!({
        "total": r.total,
        "results": results,
        "expansion_truncated": r.expansion_truncated,
        "next_cursor": r.next_after.map(|d| d.to_string()),
        "snapshot": snapshot_ref(snap),
    })
}

/// Terms and encoded postings bytes per index field (scans the whole term dictionary).
pub fn index_breakdown(snap: &Snapshot) -> Value {
    use opencnpj_store::postings::Field;
    let fields = [Field::Razao, Field::Fantasia, Field::RazaoLen, Field::FantasiaLen].into_iter().chain(Field::FILTERS);
    let mut out = Map::new();
    for f in fields {
        let terms = snap.prefix_terms(f, b"", usize::MAX);
        let bytes: u64 = terms.iter().map(|(_, off)| snap.postings_size(*off)).sum();
        // Numeric-only tokens by length (e.g. CPFs embedded in MEI names).
        let mut numeric: BTreeMap<usize, (u64, u64)> = BTreeMap::new();
        for (t, off) in &terms {
            if !t.is_empty() && t.iter().all(u8::is_ascii_digit) {
                let e = numeric.entry(t.len()).or_default();
                e.0 += 1;
                e.1 += snap.postings_size(*off);
            }
        }
        let numeric: Map<String, Value> = numeric
            .into_iter()
            .filter(|(_, (n, _))| *n >= 1000)
            .map(|(l, (n, b))| (l.to_string(), json!({"terms": n, "postings_bytes": b})))
            .collect();
        out.insert(f.name().into(), json!({ "terms": terms.len(), "postings_bytes": bytes, "numeric_by_len": numeric }));
    }
    Value::Object(out)
}

pub fn info(snap: &Snapshot) -> Value {
    let sections: BTreeMap<String, u64> = snap.sections().iter().map(|s| (format!("{:?}", s.kind).to_lowercase(), s.len)).collect();
    json!({
        "path": snap.path(),
        "file_bytes": snap.file_len(),
        "format_version": snap.manifest.format_version,
        "competence": snap.manifest.competence,
        "source_snapshot": snap.manifest.source_snapshot,
        "profile": snap.manifest.profile,
        "block_codec": snap.manifest.block_codec,
        "block_target_bytes": snap.manifest.block_target_bytes,
        "zstd_dictionary_bytes": snap.manifest.zstd_dictionary_bytes,
        "counts": snap.manifest.counts,
        "sections": sections,
        "blocks": snap.blocks().len(),
        "terms": snap.term_count(),
        "sources": snap.manifest.sources,
    })
}

/// Streams documents matching `q` (CNPJ order). Returns rows written.
pub fn export(snap: &Snapshot, q: &Query, format: &str, max: u64, out: &mut dyn Write) -> Result<u64> {
    let set = search::candidates(snap, q, &Limits::DEFAULT)?.all.context("export needs a query or at least one filter")?;
    let csv = match format {
        "csv" => true,
        "ndjson" => false,
        f => bail!("unknown format {f}"),
    };
    if csv {
        let mut header = vec!["cnpj", "razao_social"];
        header.extend(Table::Estabelecimentos.row_columns().iter().map(|c| c.name));
        writeln!(out, "{}", header.join(";"))?;
    }
    let mut n = 0u64;
    let ids: Vec<u32> = set.iter().take(usize::try_from(max).unwrap_or(usize::MAX)).collect();
    for chunk in ids.chunks(4096) {
        for d in snap.docs(chunk)? {
            let Some(ei) = d.estab else { continue };
            let c = &d.company;
            let e = &c.estabelecimentos[ei];
            let razao = c.empresas.first().map_or("", |r| r[col::emp::RAZAO_SOCIAL].as_str());
            if csv {
                let mut fields = vec![estab_cnpj(&c.basico, e), razao.to_string()];
                fields.extend(e.iter().cloned());
                writeln!(out, "{}", fields.iter().map(|f| csv_quote(f)).collect::<Vec<_>>().join(";"))?;
            } else {
                let mut v = row_json(snap, Table::Estabelecimentos, e, Privacy::default());
                let m = v.as_object_mut().expect("object");
                m.insert("cnpj".into(), Value::String(estab_cnpj(&c.basico, e)));
                m.insert("razao_social".into(), Value::String(razao.into()));
                serde_json::to_writer(&mut *out, &v)?;
                out.write_all(b"\n")?;
            }
            n += 1;
        }
    }
    out.flush()?;
    Ok(n)
}

fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Writes a table in the RFB dialect (`"a";"b"`, UTF-8). Values are the exact stored strings.
pub fn reconstruct(snap: &Snapshot, table: &str, out: &Path) -> Result<u64> {
    let t = Table::ALL.into_iter().find(|t| t.name() == table).with_context(|| format!("unknown table {table}"))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(out)?);
    let mut n = 0u64;
    for i in 0..snap.blocks().len() {
        for c in snap.block_companies(i)? {
            for r in c.rows(t) {
                write!(w, "\"{}\"", c.basico)?;
                for f in r {
                    write!(w, ";\"{f}\"")?;
                }
                w.write_all(b"\n")?;
                n += 1;
            }
        }
    }
    w.flush()?;
    Ok(n)
}

// ---------------------------------------------------------------------------------------
// Semantic reconstruction test: order-independent multiset fingerprints.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Fp {
    rows: u64,
    sum: u64,
}

impl Fp {
    fn add(&mut self, h: u64) {
        self.rows += 1;
        self.sum = self.sum.wrapping_add(h);
    }

    fn merge(mut self, o: Fp) -> Fp {
        self.rows += o.rows;
        self.sum = self.sum.wrapping_add(o.sum);
        self
    }
}

fn row_hash<'a>(table: Table, fields: impl Iterator<Item = &'a str>) -> u64 {
    let mut h = Xxh3::new();
    h.update(&[table as u8]);
    for f in fields {
        h.update(&(f.len() as u64).to_le_bytes());
        h.update(f.as_bytes());
    }
    h.digest()
}

fn fingerprint_member(path: &Path, table: Table) -> Result<(Fp, u64)> {
    let mut m = opencnpj_ingest::source::Member::open(path)?;
    let mut r = m.reader()?;
    let mut fp = Fp::default();
    let mut rejected = 0u64;
    let mut pending = Vec::with_capacity(16 << 20);
    let mut buf = vec![0u8; 8 << 20];
    let mut line = 1u64;
    let mut strings: Vec<String> = Vec::new();
    loop {
        let n = r.read(&mut buf)?;
        pending.extend_from_slice(&buf[..n]);
        let eof = n == 0;
        let cut = if eof {
            pending.len()
        } else {
            opencnpj_ingest::parse::chunk_boundary(&pending, pending.len().saturating_sub(1 << 20)).unwrap_or(0)
        };
        if cut == 0 && !eof {
            continue;
        }
        let data: Vec<u8> = pending.drain(..cut).collect();
        let lines = memchr_count(&data);
        opencnpj_ingest::parse::for_each_record(&data, table.columns().len(), line, |rec| match rec {
            Ok((_, raw, fields)) => {
                if fields[0].len() != 8 || !fields[0].is_ascii() {
                    let _ = raw;
                    rejected += 1;
                    return;
                }
                strings.clear();
                for f in fields {
                    let mut s = String::new();
                    opencnpj_core::text::latin1_to_string(f, &mut s);
                    strings.push(s);
                }
                fp.add(row_hash(table, strings.iter().map(String::as_str)));
            }
            Err(_) => rejected += 1,
        });
        line += lines;
        if eof {
            break;
        }
    }
    Ok((fp, rejected))
}

fn memchr_count(b: &[u8]) -> u64 {
    b.iter().filter(|&&c| c == b'\n').count() as u64
}

/// Re-reads every source ZIP and checks that the snapshot holds exactly the same row
/// multiset per table (plus the rejected lines), using 64-bit hash sums.
pub fn verify_against_sources(snap: &Snapshot, dir: &Path, workers: usize, log: &dyn Fn(&str)) -> Result<()> {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(workers).build()?;
    let sources = opencnpj_ingest::source::discover(dir)?;
    for s in &sources {
        let m = snap.manifest.sources.iter().find(|m| m.path == s.file_name).with_context(|| format!("{} not in manifest", s.file_name))?;
        if m.bytes != s.bytes {
            bail!("{}: size {} differs from manifest {}", s.file_name, s.bytes, m.bytes);
        }
    }
    let (src_fp, snap_fp) = pool.install(|| -> Result<_> {
        let src: Vec<(Table, Fp, u64)> = sources
            .par_iter()
            .filter_map(|s| match s.kind {
                opencnpj_ingest::source::SourceKind::Fact(t) => Some((t, s)),
                _ => None,
            })
            .map(|(t, s)| fingerprint_member(&s.path, t).map(|(fp, rj)| (t, fp, rj)))
            .collect::<Result<_>>()?;
        let blocks: Vec<[Fp; 4]> = (0..snap.blocks().len())
            .into_par_iter()
            .map(|i| -> Result<[Fp; 4]> {
                let mut fps = [Fp::default(); 4];
                for c in snap.block_companies(i)? {
                    for t in Table::ALL {
                        for r in c.rows(t) {
                            fps[t.index()].add(row_hash(t, std::iter::once(c.basico.as_str()).chain(r.iter().map(String::as_str))));
                        }
                    }
                }
                Ok(fps)
            })
            .collect::<Result<_>>()?;
        Ok((src, blocks))
    })?;
    let mut ok = true;
    let rejects = snap.rejects()?.len() as u64;
    let mut src_rejected = 0;
    for t in Table::ALL {
        let s = src_fp.iter().filter(|x| x.0 == t).fold(Fp::default(), |a, x| a.merge(x.1));
        src_rejected += src_fp.iter().filter(|x| x.0 == t).map(|x| x.2).sum::<u64>();
        let d = snap_fp.iter().fold(Fp::default(), |a, x| a.merge(x[t.index()]));
        let same = s == d;
        ok &= same;
        log(&format!(
            "{:<17} source rows {:>11}  snapshot rows {:>11}  fingerprint {}",
            t.name(),
            s.rows,
            d.rows,
            if same { "MATCH" } else { "DIFFERENT" }
        ));
    }
    let ref_rejects =
        snap.rejects()?.iter().filter(|r| !snap.manifest.sources.iter().any(|s| s.path == r.source && is_fact(&s.path))).count() as u64;
    log(&format!("rejected lines: source {src_rejected}, snapshot {} (+{ref_rejects} in reference tables)", rejects - ref_rejects));
    if !ok || src_rejected != rejects - ref_rejects {
        bail!("snapshot does not reproduce the source rows");
    }
    Ok(())
}

fn is_fact(name: &str) -> bool {
    Table::ALL.iter().any(|t| name.starts_with(t.zip_prefix()))
}
