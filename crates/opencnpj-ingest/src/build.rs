//! Snapshot build: partition (phase A) → per-bucket sort/join/encode (phase B) → assembly
//! (phase C). Phase A output is persisted in the temp directory so experiments can rebuild
//! with other profiles without re-reading the ZIPs.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use opencnpj_core::codec::{ColumnCodec, Dictionary, parse_cnae_list};
use opencnpj_core::schema::col;
use opencnpj_core::text::{latin1_to_string, tokenize};
use opencnpj_core::varint::{self, Reader};
use opencnpj_core::{Company, Kind, Table};
use opencnpj_store::block::{BlockBuilder, Codecs};
use opencnpj_store::compress::{BlockCodec, BlockCompressor, train_dictionary};
use opencnpj_store::format::SectionKind;
use opencnpj_store::manifest::{ColumnSchema, Counts, Manifest, SourceFile, TableSchema};
use opencnpj_store::postings::{Field, MAX_NAME_LEN_BUCKET, TermCollector};
use opencnpj_store::writer::{BlockMeta, SnapshotWriter};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use crate::Error;
use crate::partition::{self, PartitionConfig, bucket_path};
use crate::source::{self, Source, SourceKind};

/// Storage/speed trade-off presets. All profiles store identical semantic content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    pub codec: BlockCodec,
    /// Uncompressed bytes per block (blocks always hold whole companies).
    pub block_target: usize,
    /// Trained zstd dictionary size; 0 disables it.
    pub zstd_dict_bytes: usize,
}

impl Profile {
    pub fn preset(name: &str) -> Option<Self> {
        let (codec, block_target, zstd_dict_bytes) = match name {
            "compact" => (BlockCodec::Zstd { level: 19 }, 256 << 10, 0),
            "balanced" => (BlockCodec::Zstd { level: 9 }, 64 << 10, 112 << 10),
            "fast" => (BlockCodec::Zstd { level: 3 }, 16 << 10, 112 << 10),
            _ => return None,
        };
        Some(Self { name: name.into(), codec, block_target, zstd_dict_bytes })
    }
}

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub tmp: PathBuf,
    pub competence: String,
    pub profile: Profile,
    pub workers: usize,
    pub readers: usize,
    /// Keep phase A output for later rebuilds.
    pub keep_partition: bool,
    /// Only build buckets with `id % n == 0` (deterministic sample for experiments).
    pub sample_every: Option<u32>,
    /// Store all columns as plain text (experiment: measures the typed codecs).
    pub plain_columns: bool,
}

/// Persisted phase A result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionState {
    pub input: PathBuf,
    pub buckets: BTreeMap<u32, [bool; 4]>,
    pub dictionaries: Vec<Vec<Option<Vec<String>>>>,
    pub sources: Vec<SourceFile>,
    pub source_snapshot: Option<String>,
    pub refs: BTreeMap<String, Vec<[String; 2]>>,
    pub rejects: Vec<RejectRecord>,
    pub rows: BTreeMap<String, u64>,
    pub bytes_in: u64,
    pub partition_s: f64,
    pub hash_s: f64,
    pub temp_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectRecord {
    pub source: String,
    pub line: u64,
    pub reason: String,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildReport {
    pub output: PathBuf,
    pub profile: Option<Profile>,
    pub sha256: String,
    pub file_bytes: u64,
    pub section_bytes: BTreeMap<String, u64>,
    pub counts: Counts,
    pub timings_s: BTreeMap<String, f64>,
    pub source_zip_bytes: u64,
    pub source_csv_bytes: u64,
    pub peak_temp_bytes: u64,
    pub peak_rss_bytes: u64,
    pub workers: usize,
}

/// Minimum occurrences for a value to enter a global dictionary.
const DICT_MIN_COUNT: u64 = 2;
const DICT_MAX_VALUES: usize = 1 << 20;

pub fn partition_state_path(tmp: &Path) -> PathBuf {
    tmp.join("partition.json")
}

/// Phase A (+ hashing and reference tables). Writes `partition.json`.
pub fn partition(input: &Path, tmp: &Path, workers: usize, readers: usize, log: &(dyn Fn(&str) + Sync)) -> Result<PartitionState, Error> {
    let sources = source::discover(input)?;
    for t in Table::ALL {
        if !sources.iter().any(|s| s.kind == SourceKind::Fact(t)) {
            return Err(Error::Source(format!("no {}*.zip found in {}", t.zip_prefix(), input.display())));
        }
    }
    if tmp.exists() {
        fs::remove_dir_all(tmp)?;
    }
    fs::create_dir_all(tmp)?;
    log(&format!("phase A: {} source files, {} workers, {} readers", sources.len(), workers, readers));

    // Hash sources concurrently with parsing (sidecar `.sha256` files are reused when fresh).
    let t_hash = Instant::now();
    let (hashes, part) = rayon::join(
        || sources.par_iter().map(source_hash).collect::<Result<Vec<_>, _>>(),
        || {
            let last = AtomicU64::new(0);
            let progress = |bytes: u64| {
                let gib = bytes >> 30;
                if last.fetch_max(gib, Ordering::Relaxed) < gib {
                    log(&format!("phase A: {gib} GiB of CSV parsed"));
                }
            };
            partition::run(&sources, tmp, &PartitionConfig { workers, readers }, &progress)
        },
    );
    let hash_s = t_hash.elapsed().as_secs_f64();
    let hashes = hashes?;
    let part = part?;

    let mut refs = BTreeMap::new();
    let mut ref_rejects = Vec::new();
    for s in &sources {
        if let SourceKind::Ref(r) = s.kind {
            let (rows, rejects) = source::read_ref_table(&s.path)?;
            for rj in rejects {
                ref_rejects.push(RejectRecord { source: s.file_name.clone(), line: rj.line, reason: rj.reason.into(), raw: rj.raw });
            }
            refs.insert(r.name().to_string(), rows);
        }
    }

    let mut rows: BTreeMap<String, u64> = BTreeMap::new();
    let mut files = Vec::new();
    let mut snapshot = None;
    for (i, s) in sources.iter().enumerate() {
        let st = &part.sources[i];
        let mut member = st.member.clone();
        let mut member_bytes = Some(st.member_bytes);
        let mut nrows = st.rows;
        match s.kind {
            SourceKind::Fact(t) => *rows.entry(t.name().to_string()).or_default() += st.rows,
            SourceKind::Ref(r) => {
                let m = source::Member::open(&s.path)?;
                member = Some(m.name.clone());
                member_bytes = Some(m.declared_size);
                nrows = refs.get(r.name()).map_or(0, |v| v.len() as u64);
            }
        }
        if snapshot.is_none() {
            snapshot = st.snapshot_id.clone();
        }
        files.push(SourceFile {
            path: s.file_name.clone(),
            bytes: s.bytes,
            sha256: hashes[i].clone(),
            member,
            member_bytes,
            rows: nrows,
            rejected_lines: st.rejected + ref_rejects.iter().filter(|r| r.source == s.file_name).count() as u64,
        });
    }

    let dictionaries = choose_dictionaries(part.dicts);
    let mut rejects: Vec<RejectRecord> = part
        .rejects
        .into_iter()
        .map(|(src, r)| RejectRecord { source: sources[src].file_name.clone(), line: r.line, reason: r.reason.into(), raw: r.raw })
        .collect();
    rejects.extend(ref_rejects);
    let temp_bytes = dir_bytes(tmp)?;
    let state = PartitionState {
        input: input.to_path_buf(),
        buckets: part.buckets,
        dictionaries,
        sources: files,
        source_snapshot: snapshot,
        refs,
        rejects,
        rows,
        bytes_in: part.bytes_in,
        partition_s: part.elapsed_s,
        hash_s,
        temp_bytes,
    };
    serde_json::to_writer(BufWriter::new(File::create(partition_state_path(tmp))?), &state).map_err(|e| Error::Source(e.to_string()))?;
    Ok(state)
}

fn source_hash(s: &Source) -> Result<String, Error> {
    let sidecar = s.path.with_extension("zip.sha256");
    if let (Ok(meta_zip), Ok(meta_side)) = (fs::metadata(&s.path), fs::metadata(&sidecar))
        && meta_side.modified()? >= meta_zip.modified()?
    {
        let text = fs::read_to_string(&sidecar)?;
        if let Some(h) = text.split_whitespace().next().filter(|h| h.len() == 64) {
            return Ok(h.to_string());
        }
    }
    source::sha256_file(&s.path)
}

fn choose_dictionaries(counts: partition::DictCounts) -> Vec<Vec<Option<Vec<String>>>> {
    Table::ALL
        .iter()
        .map(|t| {
            t.row_columns()
                .iter()
                .enumerate()
                .map(|(c, column)| {
                    if column.kind != Kind::Dict {
                        return None;
                    }
                    let mut v: Vec<(&Vec<u8>, &u64)> =
                        counts[t.index()][c].iter().filter(|(k, n)| **n >= DICT_MIN_COUNT && !k.is_empty()).collect();
                    v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
                    v.truncate(DICT_MAX_VALUES);
                    Some(
                        v.into_iter()
                            .map(|(k, _)| {
                                let mut s = String::new();
                                latin1_to_string(k, &mut s);
                                s
                            })
                            .collect(),
                    )
                })
                .collect()
        })
        .collect()
}

/// Column codecs; `plain` stores every column as text (experiment baseline).
fn codecs_from(dicts: &[Vec<Option<Vec<String>>>], plain: bool) -> Codecs {
    let mut tables: [Vec<ColumnCodec>; 4] = Default::default();
    for t in Table::ALL {
        tables[t.index()] = t
            .row_columns()
            .iter()
            .zip(&dicts[t.index()])
            .map(|(c, d)| if plain { ColumnCodec::new(Kind::Text, None) } else { ColumnCodec::new(c.kind, d.clone().map(Dictionary::new)) })
            .collect();
    }
    Codecs { tables }
}

fn dir_bytes(p: &Path) -> Result<u64, Error> {
    let mut n = 0;
    for e in fs::read_dir(p)? {
        let e = e?;
        let m = e.metadata()?;
        n += if m.is_dir() { dir_bytes(&e.path())? } else { m.len() };
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------
// Phase B
// ---------------------------------------------------------------------------------------

struct RowRef {
    key: [u8; 8],
    sub: [u8; 8],
    src: u32,
    line: u64,
    off: usize,
}

/// Parses a bucket arena into row references (fields start at `off`).
fn index_rows(arena: &[u8], table: Table) -> Result<Vec<RowRef>, Error> {
    let nf = table.row_columns().len();
    let mut r = Reader::new(arena);
    let mut out = Vec::new();
    while !r.is_empty() {
        let key: [u8; 8] = r.lp_bytes()?.try_into().map_err(|_| Error::Source("bad temp key".into()))?;
        let src = r.varint()? as u32;
        let line = r.varint()?;
        let off = r.position();
        let mut sub = [0u8; 8];
        for i in 0..nf {
            let f = r.lp_bytes()?;
            if table == Table::Estabelecimentos && i < 2 {
                let (dst, width) = if i == 0 { (&mut sub[..6], 6) } else { (&mut sub[6..], 2) };
                let n = f.len().min(width);
                dst[..n].copy_from_slice(&f[..n]);
            }
        }
        out.push(RowRef { key, sub, src, line, off });
    }
    out.sort_unstable_by_key(|a| (a.key, a.sub, a.src, a.line));
    Ok(out)
}

fn decode_row(arena: &[u8], off: usize, table: Table) -> Result<Vec<String>, Error> {
    let mut r = Reader::new(&arena[off..]);
    let n = table.row_columns().len();
    let mut row = Vec::with_capacity(n);
    for _ in 0..n {
        let mut s = String::new();
        latin1_to_string(r.lp_bytes()?, &mut s);
        row.push(s);
    }
    Ok(row)
}

#[derive(Debug, Default, Clone, Copy)]
struct BucketStats {
    companies: u64,
    docs: u32,
    no_empresa: u64,
    dup_empresa: u64,
    dup_estab: u64,
}

struct BucketOut {
    metas: Vec<BlockMeta>,
    stats: BucketStats,
    segment: PathBuf,
    run: PathBuf,
}

enum Mode<'a> {
    Build { compressor: BlockCompressor },
    Sample { blocks: &'a Mutex<Vec<Vec<u8>>>, budget: usize },
}

fn name_len(text: &str) -> usize {
    let mut n = 0;
    tokenize(text, |_| n += 1);
    n
}

/// Emits all index terms for a company's documents, starting at `doc`.
fn emit_terms(c: &Company, doc: u32, tc: &mut TermCollector) {
    let emp = c.empresas.first();
    let razao = emp.map(|e| e[col::emp::RAZAO_SOCIAL].as_str()).unwrap_or("");
    let mut razao_tokens: Vec<String> = Vec::new();
    for e in &c.empresas {
        tokenize(&e[col::emp::RAZAO_SOCIAL], |t| razao_tokens.push(t.to_string()));
    }
    razao_tokens.sort_unstable();
    razao_tokens.dedup();
    let razao_len = name_len(razao);
    let simples = c.simples.first();
    let company_terms = |tc: &mut TermCollector, d: u32| {
        for t in &razao_tokens {
            tc.add(Field::Razao, t.as_bytes(), d);
        }
        if let Some(e) = emp {
            for (f, i) in [(Field::Porte, col::emp::PORTE), (Field::Natureza, col::emp::NATUREZA)] {
                if !e[i].is_empty() {
                    tc.add(f, e[i].as_bytes(), d);
                }
            }
        }
        if let Some(s) = simples {
            for (f, i) in [(Field::Simples, col::sim::OPCAO_SIMPLES), (Field::Mei, col::sim::OPCAO_MEI)] {
                if !s[i].is_empty() {
                    tc.add(f, s[i].to_ascii_uppercase().as_bytes(), d);
                }
            }
        }
    };
    let razao_len = razao_len.min(MAX_NAME_LEN_BUCKET as usize).to_string();
    if c.estabelecimentos.is_empty() {
        company_terms(tc, doc);
        tc.add(Field::RazaoLen, razao_len.as_bytes(), doc);
        return;
    }
    for (i, e) in c.estabelecimentos.iter().enumerate() {
        let d = doc + i as u32;
        company_terms(tc, d);
        tc.add(Field::RazaoLen, razao_len.as_bytes(), d);
        let mut flen = 0usize;
        tokenize(&e[col::est::NOME_FANTASIA], |t| {
            tc.add(Field::Fantasia, t.as_bytes(), d);
            flen += 1;
        });
        if flen > 0 {
            tc.add(Field::FantasiaLen, flen.min(MAX_NAME_LEN_BUCKET as usize).to_string().as_bytes(), d);
        }
        for (f, idx) in [
            (Field::Uf, col::est::UF),
            (Field::Municipio, col::est::MUNICIPIO),
            (Field::Cnae, col::est::CNAE_PRINCIPAL),
            (Field::Situacao, col::est::SITUACAO),
            (Field::MatrizFilial, col::est::MATRIZ_FILIAL),
        ] {
            let v = e[idx].trim();
            if !v.is_empty() {
                let v = if f == Field::Uf { v.to_ascii_uppercase() } else { v.to_string() };
                tc.add(f, v.as_bytes(), d);
            }
        }
        if let Some(items) = parse_cnae_list(&e[col::est::CNAE_SECUNDARIA]) {
            for code in items {
                tc.add(Field::CnaeSecundaria, format!("{code:07}").as_bytes(), d);
            }
        }
    }
}

fn build_bucket(
    tmp: &Path,
    bucket: u32,
    present: [bool; 4],
    codecs: &Codecs,
    target: usize,
    mut mode: Mode<'_>,
) -> Result<Option<BucketOut>, Error> {
    let mut arenas: [Vec<u8>; 4] = Default::default();
    let mut rows: [Vec<RowRef>; 4] = Default::default();
    for t in Table::ALL {
        if present[t.index()] {
            arenas[t.index()] = partition::read_frames(&bucket_path(tmp, t, bucket))?;
            rows[t.index()] = index_rows(&arenas[t.index()], t)?;
        }
    }
    let mut pos = [0usize; 4];
    let mut builder = BlockBuilder::new(codecs);
    let mut tc = TermCollector::default();
    let mut stats = BucketStats::default();
    let mut metas = Vec::new();
    let segment = tmp.join("seg").join(format!("{bucket:06x}.seg"));
    let run = tmp.join("runs").join(format!("{bucket:06x}.run"));
    let mut seg_file = match &mode {
        Mode::Build { .. } => Some(BufWriter::with_capacity(1 << 20, File::create(&segment)?)),
        Mode::Sample { .. } => None,
    };
    let mut block_first_doc = 0u32;
    let mut flush =
        |builder: &mut BlockBuilder<'_>, mode: &mut Mode<'_>, metas: &mut Vec<BlockMeta>, first_doc: &mut u32| -> Result<bool, Error> {
            let (companies, docs, first_key) = (builder.companies(), builder.docs(), builder.first_key());
            let raw = builder.finish();
            match mode {
                Mode::Build { compressor, .. } => {
                    let c = compressor.compress(&raw)?;
                    seg_file.as_mut().expect("segment").write_all(&c)?;
                    metas.push(BlockMeta {
                        first_key,
                        clen: c.len() as u32,
                        rlen: raw.len() as u32,
                        first_doc: *first_doc,
                        companies,
                        xxh3: xxh3_64(&c),
                    });
                }
                Mode::Sample { blocks, budget } => {
                    let mut b = blocks.lock().expect("lock");
                    if b.iter().map(Vec::len).sum::<usize>() >= *budget {
                        return Ok(false);
                    }
                    b.push(raw);
                }
            }
            *first_doc += docs;
            Ok(true)
        };
    loop {
        let key = Table::ALL.iter().filter_map(|t| rows[t.index()].get(pos[t.index()]).map(|r| r.key)).min();
        let Some(key) = key else { break };
        // Phase A only accepts 8-byte ASCII keys.
        let basico = String::from_utf8(key.to_vec()).map_err(|_| Error::Source("non-ASCII key in temp data".into()))?;
        let mut c = Company { basico, ..Default::default() };
        for t in Table::ALL {
            let rs = &rows[t.index()];
            let p = &mut pos[t.index()];
            let start = *p;
            while *p < rs.len() && rs[*p].key == key {
                *p += 1;
            }
            for (k, r) in rs[start..*p].iter().enumerate() {
                if t == Table::Estabelecimentos && k > 0 && rs[start + k - 1].sub == r.sub {
                    stats.dup_estab += 1;
                }
                c.rows_mut(t).push(decode_row(&arenas[t.index()], r.off, t)?);
            }
        }
        match c.empresas.len() {
            0 => stats.no_empresa += 1,
            1 => {}
            n => stats.dup_empresa += n as u64 - 1,
        }
        if matches!(mode, Mode::Build { .. }) {
            emit_terms(&c, stats.docs, &mut tc);
        }
        builder.push(&c);
        stats.companies += 1;
        stats.docs += c.doc_count();
        if builder.raw_len() >= target && !flush(&mut builder, &mut mode, &mut metas, &mut block_first_doc)? {
            return Ok(None);
        }
    }
    if !builder.is_empty() {
        flush(&mut builder, &mut mode, &mut metas, &mut block_first_doc)?;
    }
    drop(arenas);
    match mode {
        Mode::Sample { .. } => Ok(None),
        Mode::Build { .. } => {
            seg_file.take().expect("segment").into_inner().map_err(|e| Error::Io(e.into_error()))?.sync_data()?;
            tc.write_run(File::create(&run)?)?;
            Ok(Some(BucketOut { metas, stats, segment, run }))
        }
    }
}

/// Phases B and C from a persisted partition.
pub fn build_from_partition(state: &PartitionState, cfg: &BuildConfig, log: &(dyn Fn(&str) + Sync)) -> Result<BuildReport, Error> {
    let tmp = &cfg.tmp;
    let profile = &cfg.profile;
    let codecs = codecs_from(&state.dictionaries, cfg.plain_columns);
    let buckets: Vec<(u32, [bool; 4])> =
        state.buckets.iter().filter(|(b, _)| cfg.sample_every.is_none_or(|n| *b % n == 0)).map(|(b, p)| (*b, *p)).collect();
    let mut timings = BTreeMap::new();
    timings.insert("partition".to_string(), state.partition_s);
    timings.insert("hash_sources".to_string(), state.hash_s);

    // zstd dictionary from a deterministic sample of buckets.
    let t_dict = Instant::now();
    let zdict = if profile.zstd_dict_bytes > 0 && matches!(profile.codec, BlockCodec::Zstd { .. }) {
        let blocks = Mutex::new(Vec::new());
        let step = (buckets.len() / 24).max(1);
        let budget = (profile.zstd_dict_bytes * 100).max(8 << 20);
        let sample: Vec<_> = buckets.iter().step_by(step).collect();
        sample.par_iter().try_for_each(|(b, p)| {
            build_bucket(tmp, *b, *p, &codecs, profile.block_target, Mode::Sample { blocks: &blocks, budget }).map(|_| ())
        })?;
        let mut samples = blocks.into_inner().expect("lock");
        samples.sort();
        let d = train_dictionary(&samples, profile.zstd_dict_bytes)?;
        log(&format!("trained zstd dictionary: {} bytes from {} sample blocks", d.len(), samples.len()));
        Some(d)
    } else {
        None
    };
    timings.insert("train_dictionary".to_string(), t_dict.elapsed().as_secs_f64());

    fs::create_dir_all(tmp.join("seg"))?;
    fs::create_dir_all(tmp.join("runs"))?;
    let t_b = Instant::now();
    let done = AtomicU64::new(0);
    let temp_now = AtomicU64::new(state.temp_bytes);
    let temp_peak = AtomicU64::new(state.temp_bytes);
    let total = buckets.len() as u64;
    let outs: Vec<BucketOut> = buckets
        .par_iter()
        .with_max_len(1)
        .map(|(b, p)| -> Result<BucketOut, Error> {
            let compressor = BlockCompressor::new(profile.codec, zdict.as_deref())?;
            let out = build_bucket(tmp, *b, *p, &codecs, profile.block_target, Mode::Build { compressor })?.expect("build mode");
            let written = fs::metadata(&out.segment)?.len() + fs::metadata(&out.run)?.len();
            let now = temp_now.fetch_add(written, Ordering::Relaxed) + written;
            temp_peak.fetch_max(now, Ordering::Relaxed);
            if !cfg.keep_partition {
                for t in Table::ALL {
                    if p[t.index()] {
                        let path = bucket_path(tmp, t, *b);
                        let n = fs::metadata(&path)?.len();
                        fs::remove_file(path)?;
                        temp_now.fetch_sub(n, Ordering::Relaxed);
                    }
                }
            }
            let d = done.fetch_add(1, Ordering::Relaxed) + 1;
            if d.is_multiple_of(100) || d == total {
                log(&format!("phase B: {d}/{total} buckets"));
            }
            Ok(out)
        })
        .collect::<Result<_, _>>()?;
    timings.insert("encode_blocks".to_string(), t_b.elapsed().as_secs_f64());

    // Phase C: assemble.
    let t_c = Instant::now();
    let flags = 0;
    let mut w = SnapshotWriter::create(&cfg.output, &cfg.competence, flags)?;
    let mut counts = Counts { rows: state.rows.clone(), rejected_lines: state.rejects.len() as u64, ..Default::default() };
    let mut runs = Vec::with_capacity(outs.len());
    let mut doc_base = 0u32;
    for o in &outs {
        w.append_segment(&o.segment, &o.metas, doc_base)?;
        runs.push((o.run.clone(), doc_base));
        counts.companies += o.stats.companies;
        counts.companies_without_empresa += o.stats.no_empresa;
        counts.duplicate_empresa_keys += o.stats.dup_empresa;
        counts.duplicate_estabelecimento_keys += o.stats.dup_estab;
        doc_base = doc_base.checked_add(o.stats.docs).ok_or_else(|| Error::Source("more than 2^32 documents".into()))?;
    }
    counts.documents = u64::from(doc_base);
    counts.blocks = w.blocks().len() as u64;
    timings.insert("assemble_data".to_string(), t_c.elapsed().as_secs_f64());

    let t_idx = Instant::now();
    let fst_path = tmp.join("terms.fst");
    let post_path = tmp.join("postings.bin");
    let idx = opencnpj_store::index::merge_runs(&runs, &fst_path, &post_path)?;
    counts.terms = idx.terms;
    timings.insert("merge_index".to_string(), t_idx.elapsed().as_secs_f64());
    temp_peak.fetch_max(temp_now.load(Ordering::Relaxed) + idx.fst_bytes + idx.postings_bytes, Ordering::Relaxed);

    let t_fin = Instant::now();
    let dict_bytes = opencnpj_store::reader::encode_dictionaries(&codecs);
    w.section(SectionKind::Dictionaries, &dict_bytes, true)?;
    if let Some(d) = &zdict {
        w.section(SectionKind::ZstdDict, d, false)?;
    }
    w.section(SectionKind::RefTables, &serde_json::to_vec(&state.refs).map_err(|e| Error::Source(e.to_string()))?, true)?;
    let mut rj = Vec::new();
    varint::put(&mut rj, state.rejects.len() as u64);
    for r in &state.rejects {
        varint::put_lp(&mut rj, r.source.as_bytes());
        varint::put(&mut rj, r.line);
        varint::put_lp(&mut rj, r.reason.as_bytes());
        varint::put_lp(&mut rj, &r.raw);
    }
    w.section(SectionKind::Rejects, &rj, true)?;
    w.section_from_file(SectionKind::Terms, &fst_path)?;
    w.section_from_file(SectionKind::Postings, &post_path)?;

    let manifest = Manifest {
        format: "opencnpj-snapshot".into(),
        format_version: [opencnpj_store::format::FORMAT_MAJOR, opencnpj_store::format::FORMAT_MINOR],
        competence: cfg.competence.clone(),
        source_snapshot: state.source_snapshot.clone(),
        created_by: format!("opencnpj {}", env!("CARGO_PKG_VERSION")),
        profile: profile.name.clone(),
        block_codec: profile.codec,
        block_target_bytes: profile.block_target,
        zstd_dictionary_bytes: zdict.as_ref().map_or(0, Vec::len),
        schema: Table::ALL
            .iter()
            .map(|t| TableSchema {
                table: t.name().into(),
                columns: t
                    .row_columns()
                    .iter()
                    .zip(codecs.table(*t))
                    .map(|(c, codec)| ColumnSchema {
                        name: c.name.into(),
                        kind: codec.kind,
                        dictionary_len: codec.dict.as_ref().map(|d| d.values.len()),
                    })
                    .collect(),
            })
            .collect(),
        sources: state.sources.clone(),
        counts: counts.clone(),
    };
    w.section(SectionKind::Manifest, &serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Source(e.to_string()))?, false)?;
    let written = w.finish()?;
    timings.insert("finalize".to_string(), t_fin.elapsed().as_secs_f64());
    fs::write(cfg.output.with_extension("ocnpj.sha256"), format!("{}  {}\n", written.sha256, file_name(&cfg.output)))?;

    for o in &outs {
        let _ = fs::remove_file(&o.segment);
        let _ = fs::remove_file(&o.run);
    }
    let _ = fs::remove_file(&fst_path);
    let _ = fs::remove_file(&post_path);

    let mut section_bytes = BTreeMap::new();
    for s in &written.sections {
        section_bytes.insert(format!("{:?}", s.kind).to_lowercase(), s.len);
    }
    counts.section_bytes = section_bytes.clone();
    Ok(BuildReport {
        output: written.path,
        profile: Some(profile.clone()),
        sha256: written.sha256,
        file_bytes: written.bytes,
        section_bytes,
        counts,
        timings_s: timings,
        source_zip_bytes: state.sources.iter().map(|s| s.bytes).sum(),
        source_csv_bytes: state.bytes_in,
        peak_temp_bytes: temp_peak.load(Ordering::Relaxed),
        peak_rss_bytes: peak_rss(),
        workers: cfg.workers,
    })
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Peak resident set size of this process (Linux `VmHWM`), 0 elsewhere.
pub fn peak_rss() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
        .map_or(0, |kb| kb * 1024)
}

/// Full build: partition then encode/assemble.
pub fn build(cfg: &BuildConfig, log: &(dyn Fn(&str) + Sync)) -> Result<BuildReport, Error> {
    let state = partition(&cfg.input, &cfg.tmp, cfg.workers, cfg.readers, log)?;
    log(&format!(
        "phase A done in {:.1}s: {} rows, {} rejected lines, {:.2} GiB CSV",
        state.partition_s,
        state.rows.values().sum::<u64>(),
        state.rejects.len(),
        state.bytes_in as f64 / f64::from(1 << 30)
    ));
    let report = build_from_partition(&state, cfg, log)?;
    if !cfg.keep_partition {
        let _ = fs::remove_dir_all(&cfg.tmp);
    }
    Ok(report)
}

pub fn load_partition(tmp: &Path) -> Result<PartitionState, Error> {
    serde_json::from_reader(std::io::BufReader::new(File::open(partition_state_path(tmp))?)).map_err(|e| Error::Source(e.to_string()))
}

/// Infers `YYYY-MM` from a directory name like `.../2026-10`.
pub fn competence_from_dir(dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_str()?;
    let ok = name.len() == 7
        && name.as_bytes()[4] == b'-'
        && name[..4].chars().all(|c| c.is_ascii_digit())
        && name[5..].chars().all(|c| c.is_ascii_digit());
    ok.then(|| name.to_string())
}
