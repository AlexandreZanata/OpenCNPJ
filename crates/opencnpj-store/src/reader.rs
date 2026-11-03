//! Read-only snapshot access.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use memmap2::Mmap;
use opencnpj_core::codec::{ColumnCodec, Dictionary};
use opencnpj_core::varint::Reader;
use opencnpj_core::{Company, Kind, Table};
use roaring::RoaringBitmap;
use xxhash_rust::xxh3::xxh3_64;

use crate::Error;
use crate::block::{BlockView, Codecs, Select};
use crate::compress::BlockDecompressor;
use crate::format::*;
use crate::manifest::Manifest;
use crate::postings::{self, Field};

/// Dictionary terms (field byte stripped) with their postings offsets.
pub type TermList = Vec<(Vec<u8>, u64)>;

/// Block index, raw bytes, keys and per-company row counts.
type ParsedBlock = (usize, Vec<u8>, Vec<String>, Vec<[u32; 4]>);

/// Byte range of the shared mapping, usable as FST storage.
#[derive(Clone)]
pub struct MapSlice {
    map: Arc<Mmap>,
    start: usize,
    end: usize,
}

impl AsRef<[u8]> for MapSlice {
    fn as_ref(&self) -> &[u8] {
        &self.map[self.start..self.end]
    }
}

pub struct Snapshot {
    path: PathBuf,
    map: Arc<Mmap>,
    pub header: Header,
    pub manifest: Manifest,
    sections: Vec<SectionEntry>,
    blocks: Vec<BlockEntry>,
    pub codecs: Codecs,
    decompressor: BlockDecompressor,
    terms: fst::Map<MapSlice>,
    postings: (usize, usize),
    pub refs: BTreeMap<String, Vec<[String; 2]>>,
    /// `table → code → row index` (reference rows keep their source order).
    ref_index: HashMap<String, HashMap<String, usize>>,
    doc_count: u32,
    cache: Mutex<PostingsCache>,
}

/// Postings smaller than this are cheap to decode and never cached.
const CACHE_MIN_ENCODED: usize = 16 << 10;
pub const DEFAULT_POSTINGS_CACHE: usize = 256 << 20;

/// FIFO cache of decoded postings bounded by encoded bytes.
struct PostingsCache {
    map: HashMap<u64, (Arc<RoaringBitmap>, usize)>,
    order: VecDeque<u64>,
    bytes: usize,
    budget: usize,
}

impl PostingsCache {
    fn get(&self, off: u64) -> Option<Arc<RoaringBitmap>> {
        self.map.get(&off).map(|e| e.0.clone())
    }

    fn insert(&mut self, off: u64, bm: Arc<RoaringBitmap>, len: usize) {
        if len > self.budget || self.map.contains_key(&off) {
            return;
        }
        self.map.insert(off, (bm, len));
        self.order.push_back(off);
        self.bytes += len;
        self.evict();
    }

    fn evict(&mut self) {
        while self.bytes > self.budget {
            let Some(old) = self.order.pop_front() else { break };
            if let Some((_, n)) = self.map.remove(&old) {
                self.bytes -= n;
            }
        }
    }
}

/// One search document resolved to its company and establishment.
#[derive(Debug, Clone)]
pub struct DocRecord {
    pub doc: u32,
    pub company: Company,
    /// Index into `company.estabelecimentos` (which holds only this document's row),
    /// `None` for a company without establishments. Partners are not decoded.
    pub estab: Option<usize>,
}

/// Reconstructed rejected source line (kept verbatim, never dropped).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Reject {
    pub source: String,
    pub line: u64,
    pub reason: String,
    pub raw_latin1: Vec<u8>,
}

impl Snapshot {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let file = File::open(path)?;
        // SAFETY: published snapshots are immutable: the builder writes a temporary file and
        // renames it into place, and nothing in this project opens a published snapshot for
        // writing. Truncating or rewriting the file externally while it is mapped is outside
        // the supported contract (it can raise SIGBUS); `verify` detects any tampering.
        let map = Arc::new(unsafe { Mmap::map(&file)? });
        let len = map.len();
        if len < HEADER_LEN + FOOTER_LEN {
            return Err(Error::Corrupt("file too small"));
        }
        let header = Header::decode(&map[..HEADER_LEN])?;
        let footer = Footer::decode(&map[len - FOOTER_LEN..])?;
        if footer.file_len != len as u64 {
            return Err(Error::Corrupt("file length does not match footer (truncated?)"));
        }
        let t_start = usize::try_from(footer.table_offset).map_err(|_| Error::Corrupt("table offset"))?;
        let t_end = t_start.checked_add(footer.table_len as usize).ok_or(Error::Corrupt("table offset"))?;
        if t_start < HEADER_LEN || t_end != len - FOOTER_LEN {
            return Err(Error::Corrupt("section table out of bounds"));
        }
        let table = &map[t_start..t_end];
        if xxh3_64(table) != footer.table_xxh3 {
            return Err(Error::Corrupt("section table checksum mismatch"));
        }
        let sections = decode_sections(table, footer.table_offset)?;

        let get = |kind| section_bytes(&map, &sections, kind, true);
        let manifest: Manifest = serde_json::from_slice(&get(SectionKind::Manifest)?.ok_or(Error::Corrupt("missing manifest"))?)
            .map_err(|_| Error::Corrupt("manifest json"))?;
        let blocks = BlockEntry::decode_all(&get(SectionKind::BlockDir)?.ok_or(Error::Corrupt("missing block directory"))?)?;
        let codecs = decode_dictionaries(&get(SectionKind::Dictionaries)?.ok_or(Error::Corrupt("missing dictionaries"))?)?;
        let decompressor = BlockDecompressor::new(manifest.block_codec, get(SectionKind::ZstdDict)?.as_deref());
        let refs: BTreeMap<String, Vec<[String; 2]>> = match get(SectionKind::RefTables)? {
            Some(r) => serde_json::from_slice(&r).map_err(|_| Error::Corrupt("reference tables json"))?,
            None => BTreeMap::new(),
        };
        let find = |kind| sections.iter().find(|s| s.kind == kind).copied();
        let t = find(SectionKind::Terms).ok_or(Error::Corrupt("missing terms section"))?;
        let slice = MapSlice { map: map.clone(), start: t.offset as usize, end: (t.offset + t.len) as usize };
        if xxh3_64(slice.as_ref()) != t.xxh3 {
            return Err(Error::Corrupt("terms checksum mismatch"));
        }
        let terms = fst::Map::new(slice).map_err(|_| Error::Corrupt("terms fst"))?;
        let p = find(SectionKind::Postings).ok_or(Error::Corrupt("missing postings section"))?;
        let doc_count = u32::try_from(manifest.counts.documents).map_err(|_| Error::Corrupt("document count"))?;
        let snap = Self {
            path: path.to_path_buf(),
            map,
            header,
            manifest,
            sections,
            blocks,
            codecs,
            decompressor,
            terms,
            postings: (p.offset as usize, (p.offset + p.len) as usize),
            ref_index: refs
                .iter()
                .map(|(t, rows)| (t.clone(), rows.iter().enumerate().rev().map(|(i, r)| (r[0].clone(), i)).collect()))
                .collect(),
            refs,
            doc_count,
            cache: Mutex::new(PostingsCache { map: HashMap::new(), order: VecDeque::new(), bytes: 0, budget: DEFAULT_POSTINGS_CACHE }),
        };
        snap.validate_blocks()?;
        Ok(snap)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_len(&self) -> u64 {
        self.map.len() as u64
    }

    pub fn sections(&self) -> &[SectionEntry] {
        &self.sections
    }

    pub fn blocks(&self) -> &[BlockEntry] {
        &self.blocks
    }

    pub fn doc_count(&self) -> u32 {
        self.doc_count
    }

    fn entry(&self, kind: SectionKind) -> Option<SectionEntry> {
        self.sections.iter().find(|s| s.kind == kind).copied()
    }

    fn optional(&self, kind: SectionKind) -> Result<Option<Vec<u8>>, Error> {
        section_bytes(&self.map, &self.sections, kind, true)
    }

    fn validate_blocks(&self) -> Result<(), Error> {
        let data = self.entry(SectionKind::Data).ok_or(Error::Corrupt("missing data section"))?;
        let (lo, hi) = (data.offset, data.offset + data.len);
        let mut expect_off = lo;
        for (i, b) in self.blocks.iter().enumerate() {
            if b.offset != expect_off || b.rlen as usize > MAX_BLOCK_RAW {
                return Err(Error::Corrupt("block directory inconsistent with data section"));
            }
            expect_off += u64::from(b.clen);
            if i > 0 && (b.first_key <= self.blocks[i - 1].first_key || b.first_doc < self.blocks[i - 1].first_doc) {
                return Err(Error::Corrupt("block directory not sorted"));
            }
        }
        if expect_off != hi {
            return Err(Error::Corrupt("block directory does not cover data section"));
        }
        Ok(())
    }

    /// Full integrity check: every section checksum, every block checksum and decode, and
    /// the stored counts. Returns `(companies, documents)` actually decoded.
    pub fn verify(&self, mut progress: impl FnMut(usize, usize)) -> Result<(u64, u64), Error> {
        for s in &self.sections {
            let bytes = &self.map[s.offset as usize..(s.offset + s.len) as usize];
            if xxh3_64(bytes) != s.xxh3 {
                return Err(Error::Corrupt("section checksum mismatch"));
            }
        }
        let (mut companies, mut docs) = (0u64, 0u64);
        let mut prev_key: Option<String> = None;
        for i in 0..self.blocks.len() {
            let b = &self.blocks[i];
            if docs != u64::from(b.first_doc) {
                return Err(Error::Corrupt("block first_doc does not match decoded documents"));
            }
            let all = self.block_companies(i)?;
            if all.len() != b.companies as usize {
                return Err(Error::Corrupt("block company count mismatch"));
            }
            for c in &all {
                if prev_key.as_deref().is_some_and(|p| p >= c.basico.as_str()) {
                    return Err(Error::Corrupt("companies not strictly sorted"));
                }
                prev_key = Some(c.basico.clone());
                docs += u64::from(c.doc_count());
            }
            companies += all.len() as u64;
            progress(i + 1, self.blocks.len());
        }
        if companies != self.manifest.counts.companies || docs != self.manifest.counts.documents {
            return Err(Error::Corrupt("decoded counts differ from manifest"));
        }
        Ok((companies, docs))
    }

    fn raw_block(&self, i: usize) -> Result<Vec<u8>, Error> {
        let b = self.blocks.get(i).ok_or(Error::Corrupt("block index"))?;
        let data = &self.map[b.offset as usize..b.offset as usize + b.clen as usize];
        if xxh3_64(data) != b.xxh3 {
            return Err(Error::Corrupt("block checksum mismatch"));
        }
        self.decompressor.decompress(data, b.rlen as usize)
    }

    /// Decodes every company in block `i`.
    pub fn block_companies(&self, i: usize) -> Result<Vec<Company>, Error> {
        let raw = self.raw_block(i)?;
        BlockView::parse(&raw, &self.codecs)?.all(&self.codecs)
    }

    /// Exact lookup by `cnpj_basico` (8 characters).
    pub fn company(&self, basico: &str) -> Result<Option<Company>, Error> {
        let Some((raw, pos)) = self.locate(basico)? else { return Ok(None) };
        let view = BlockView::parse(&raw, &self.codecs)?;
        let keys = view.keys()?;
        let counts = view.counts()?;
        Ok(Some(view.company(&self.codecs, pos, keys[pos].clone(), &counts)?))
    }

    /// Resolves documents (ascending ids decompress each block once).
    pub fn docs(&self, ids: &[u32]) -> Result<Vec<DocRecord>, Error> {
        let mut out = Vec::with_capacity(ids.len());
        // Parsed state of the current block: (index, raw, keys, counts).
        let mut cur: Option<ParsedBlock> = None;
        for &doc in ids {
            if doc >= self.doc_count {
                return Err(Error::Invalid(format!("doc {doc} out of range")));
            }
            let bi = self.blocks.partition_point(|b| b.first_doc <= doc) - 1;
            if cur.as_ref().map(|c| c.0) != Some(bi) {
                let raw = self.raw_block(bi)?;
                let view = BlockView::parse(&raw, &self.codecs)?;
                let (keys, counts) = (view.keys()?, view.counts()?);
                cur = Some((bi, raw, keys, counts));
            }
            let (_, raw, keys, counts) = cur.as_ref().expect("current block");
            let view = BlockView::parse(raw, &self.codecs)?;
            let mut d = self.blocks[bi].first_doc;
            let mut found = None;
            for (ci, n) in counts.iter().enumerate() {
                let docs = n[Table::Estabelecimentos.index()].max(1);
                if doc < d + docs {
                    found = Some((ci, (doc - d) as usize, n[Table::Estabelecimentos.index()] > 0));
                    break;
                }
                d += docs;
            }
            let (ci, ei, has_estab) = found.ok_or(Error::Corrupt("doc not found in its block"))?;
            let sel = Select { estab: has_estab.then_some(ei), socios: false };
            let company = view.company_select(&self.codecs, ci, keys[ci].clone(), counts, sel)?;
            out.push(DocRecord { doc, company, estab: has_estab.then_some(0) });
        }
        Ok(out)
    }

    /// Establishment lookup that decodes only the matching row (plus company rows).
    /// Returns the company (with exactly one establishment) and its establishment count.
    pub fn establishment(&self, basico: &str, ordem: &str, dv: &str) -> Result<Option<(Company, usize)>, Error> {
        let Some((raw, pos)) = self.locate(basico)? else { return Ok(None) };
        let view = BlockView::parse(&raw, &self.codecs)?;
        let keys = view.keys()?;
        let counts = view.counts()?;
        let ordens = view.estab_column(&self.codecs, pos, &counts, opencnpj_core::schema::col::est::ORDEM)?;
        let dvs = view.estab_column(&self.codecs, pos, &counts, opencnpj_core::schema::col::est::DV)?;
        let Some(k) = (0..ordens.len()).find(|&k| ordens[k] == ordem && dvs[k] == dv) else { return Ok(None) };
        let c = view.company_select(&self.codecs, pos, keys[pos].clone(), &counts, Select { estab: Some(k), socios: true })?;
        Ok(Some((c, ordens.len())))
    }

    /// Block bytes and company position for `basico`.
    fn locate(&self, basico: &str) -> Result<Option<(Vec<u8>, usize)>, Error> {
        let key = basico.as_bytes();
        if key.len() != 8 {
            return Ok(None);
        }
        let i = self.blocks.partition_point(|b| b.first_key.as_slice() <= key);
        if i == 0 {
            return Ok(None);
        }
        let raw = self.raw_block(i - 1)?;
        let view = BlockView::parse(&raw, &self.codecs)?;
        let keys = view.keys()?;
        let Ok(pos) = keys.binary_search_by(|k| k.as_bytes().cmp(key)) else { return Ok(None) };
        Ok(Some((raw, pos)))
    }

    /// Postings for an exact term.
    pub fn postings(&self, field: Field, value: &[u8]) -> Result<Option<Arc<RoaringBitmap>>, Error> {
        match self.terms.get(field.term(value)) {
            Some(off) => Ok(Some(self.postings_at(off)?)),
            None => Ok(None),
        }
    }

    fn postings_at(&self, off: u64) -> Result<Arc<RoaringBitmap>, Error> {
        if let Some(b) = self.cache.lock().expect("lock").get(off) {
            return Ok(b);
        }
        let section = &self.map[self.postings.0..self.postings.1];
        let (bm, len) = postings::decode_postings(section, off as usize, self.doc_count)?;
        let bm = Arc::new(bm);
        if len >= CACHE_MIN_ENCODED {
            self.cache.lock().expect("lock").insert(off, bm.clone(), len);
        }
        Ok(bm)
    }

    /// Byte budget for decoded large postings (hot filter values, common tokens).
    pub fn set_postings_cache_bytes(&self, bytes: usize) {
        let mut c = self.cache.lock().expect("lock");
        c.budget = bytes;
        c.evict();
    }

    /// Terms of `field` starting with `prefix`, at most `limit` (in byte order).
    pub fn prefix_terms(&self, field: Field, prefix: &[u8], limit: usize) -> Vec<(Vec<u8>, u64)> {
        use fst::{IntoStreamer, Streamer};
        let start = field.term(prefix);
        let mut end = start.clone();
        // Exclusive upper bound: increment the last byte that is not 0xFF.
        while let Some(last) = end.pop() {
            if last < 0xFF {
                end.push(last + 1);
                break;
            }
        }
        let mut s = self.terms.range().ge(&start).lt(&end).into_stream();
        let mut out = Vec::new();
        while let Some((k, v)) = s.next() {
            out.push((k[1..].to_vec(), v));
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    /// Encoded size of the postings at `off` (a proxy for term frequency), without decoding.
    pub fn postings_size(&self, off: u64) -> u64 {
        let section = &self.map[self.postings.0..self.postings.1];
        section.get(off as usize..).and_then(|b| Reader::new(b).varint().ok()).unwrap_or(0)
    }

    /// Up to `take` terms of `field` starting with `prefix`, preferring the most frequent
    /// among the first `scan` matches. Returns the terms and whether any were left out.
    pub fn prefix_terms_ranked(&self, field: Field, prefix: &[u8], take: usize, scan: usize) -> (TermList, bool) {
        let all = self.prefix_terms(field, prefix, scan);
        rank_terms(self, all, take, scan)
    }

    /// Like [`Self::fuzzy_terms`], keeping the most frequent `take` of the first `scan` matches.
    pub fn fuzzy_terms_ranked(
        &self,
        field: Field,
        value: &str,
        dist: u32,
        take: usize,
        scan: usize,
    ) -> Result<(TermList, bool), Error> {
        let all = self.fuzzy_terms(field, value, dist, scan)?;
        Ok(rank_terms(self, all, take, scan))
    }

    /// Terms within Levenshtein distance `dist` of `value`, at most `limit`.
    pub fn fuzzy_terms(&self, field: Field, value: &str, dist: u32, limit: usize) -> Result<Vec<(Vec<u8>, u64)>, Error> {
        use fst::{IntoStreamer, Streamer};
        let mut term = String::with_capacity(value.len() + 1);
        term.push(char::from(field as u8));
        term.push_str(value);
        let aut = fst::automaton::Levenshtein::new(&term, dist).map_err(|_| Error::Invalid("fuzzy automaton too large".into()))?;
        let mut s = self.terms.search(aut).into_stream();
        let mut out = Vec::new();
        while let Some((k, v)) = s.next() {
            if k.first() == Some(&(field as u8)) {
                out.push((k[1..].to_vec(), v));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn postings_by_offset(&self, off: u64) -> Result<Arc<RoaringBitmap>, Error> {
        self.postings_at(off)
    }

    /// All values of a filter field with their document counts.
    pub fn field_values(&self, field: Field) -> Result<Vec<(String, u64)>, Error> {
        self.prefix_terms(field, b"", usize::MAX)
            .into_iter()
            .map(|(v, off)| Ok((String::from_utf8_lossy(&v).into_owned(), self.postings_at(off)?.len())))
            .collect()
    }

    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    /// Rejected source lines stored in the snapshot.
    pub fn rejects(&self) -> Result<Vec<Reject>, Error> {
        let Some(b) = self.optional(SectionKind::Rejects)? else { return Ok(Vec::new()) };
        let mut r = Reader::new(&b);
        let n = r.varint_usize()?;
        let mut out = Vec::with_capacity(n.min(1 << 20));
        for _ in 0..n {
            let source = String::from_utf8_lossy(r.lp_bytes()?).into_owned();
            let line = r.varint()?;
            let reason = String::from_utf8_lossy(r.lp_bytes()?).into_owned();
            let raw_latin1 = r.lp_bytes()?.to_vec();
            out.push(Reject { source, line, reason, raw_latin1 });
        }
        Ok(out)
    }

    /// Description for a reference-table code (`cnaes`, `municipios`, ...).
    pub fn label(&self, table: &str, code: &str) -> Option<&str> {
        let i = *self.ref_index.get(table)?.get(code)?;
        Some(self.refs[table][i][1].as_str())
    }
}

/// Most frequent terms first, at most `take` terms and `MAX_EXPANSION_BYTES` of postings
/// (the first term is always kept). Bounds the work of short prefixes like `BAR`.
fn rank_terms(snap: &Snapshot, mut terms: Vec<(Vec<u8>, u64)>, take: usize, scan: usize) -> (TermList, bool) {
    let n = terms.len();
    let mut sized: Vec<(u64, Vec<u8>, u64)> = terms.drain(..).map(|(t, off)| (snap.postings_size(off), t, off)).collect();
    sized.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut budget = 0u64;
    for (i, (size, t, off)) in sized.into_iter().enumerate() {
        if i >= take || (i > 0 && budget + size > MAX_EXPANSION_BYTES) {
            break;
        }
        budget += size;
        terms.push((t, off));
    }
    let truncated = terms.len() < n || n >= scan;
    (terms, truncated)
}

/// Encoded postings bytes one expanded token may union.
pub const MAX_EXPANSION_BYTES: u64 = 24 << 20;

fn section_bytes(map: &Mmap, sections: &[SectionEntry], kind: SectionKind, check: bool) -> Result<Option<Vec<u8>>, Error> {
    let Some(s) = sections.iter().find(|s| s.kind == kind) else { return Ok(None) };
    let bytes = &map[s.offset as usize..(s.offset + s.len) as usize];
    if check && xxh3_64(bytes) != s.xxh3 {
        return Err(Error::Corrupt("section checksum mismatch"));
    }
    Ok(Some(match s.codec {
        SectionCodec::Raw => bytes.to_vec(),
        SectionCodec::Zstd => {
            if s.raw_len > 1 << 32 {
                return Err(Error::Corrupt("section raw length too large"));
            }
            let out = zstd::bulk::decompress(bytes, s.raw_len as usize).map_err(|_| Error::Corrupt("section zstd"))?;
            if out.len() as u64 != s.raw_len {
                return Err(Error::Corrupt("section raw length mismatch"));
            }
            out
        }
    }))
}

/// Serializes dictionaries: per table, per row column: `varint(0)` or `varint(n+1)` + values.
pub fn encode_dictionaries(codecs: &Codecs) -> Vec<u8> {
    let mut out = Vec::new();
    for t in Table::ALL {
        for c in codecs.table(t) {
            match &c.dict {
                None => opencnpj_core::varint::put(&mut out, 0),
                Some(d) => {
                    opencnpj_core::varint::put(&mut out, d.values.len() as u64 + 1);
                    for v in &d.values {
                        opencnpj_core::varint::put_lp(&mut out, v.as_bytes());
                    }
                }
            }
        }
    }
    out
}

fn decode_dictionaries(b: &[u8]) -> Result<Codecs, Error> {
    let mut r = Reader::new(b);
    let mut tables: [Vec<ColumnCodec>; 4] = Default::default();
    for t in Table::ALL {
        for col in t.row_columns() {
            let n = r.varint_usize()?;
            let dict = if n == 0 {
                None
            } else {
                if n - 1 > b.len() {
                    return Err(Error::Corrupt("dictionary size"));
                }
                let mut values = Vec::with_capacity(n - 1);
                for _ in 0..n - 1 {
                    values.push(String::from_utf8(r.lp_bytes()?.to_vec()).map_err(|_| Error::Corrupt("dictionary utf-8"))?);
                }
                Some(Dictionary::new(values))
            };
            if dict.is_some() != (col.kind == Kind::Dict) {
                return Err(Error::Corrupt("dictionary does not match schema"));
            }
            tables[t.index()].push(ColumnCodec::new(col.kind, dict));
        }
    }
    if !r.is_empty() {
        return Err(Error::Corrupt("trailing dictionary bytes"));
    }
    Ok(Codecs { tables })
}
