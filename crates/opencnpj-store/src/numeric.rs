//! Numeric columns: one optional integer per document (dates as day numbers, capital in
//! cents), for range filters, sorting and year histograms.
//!
//! Values are stored in blocks of [`BLOCK_DOCS`] documents with frame-of-reference
//! bit-packing (`value - min + 1`, `0` = no value). Each block keeps its min/max, so a range
//! filter skips blocks that cannot match and takes blocks that fully match without
//! decoding. Opening dates correlate with the CNPJ order, so most blocks pack in few bits.
//!
//! Section layout (`numeric`, stored raw and read in place):
//!
//! ```text
//! u32 n_columns, u32 doc_count, u32 block_docs, u32 reserved
//! n_columns × {field u8, reserved [u8; 7], n_blocks u64, dir_offset u64, data_offset u64, data_len u64} (40 B)
//! per column: n_blocks × {min u64, max u64, data_rel u64, bits u8, flags u8, reserved [u8; 6]} (32 B)
//!             then packed data (+ 8 zero bytes of padding)
//! ```
//! `flags`: bit 0 = the block has documents without a value, bit 1 = no document has one.

use std::io::Write;

use roaring::RoaringBitmap;

use crate::Error;

pub const BLOCK_DOCS: usize = 1024;
const HEADER_LEN: usize = 16;
const ENTRY_LEN: usize = 40;
const DIR_LEN: usize = 32;
const PADDING: usize = 8;
const HAS_MISSING: u8 = 1;
const ALL_MISSING: u8 = 2;

/// Numeric fields. The discriminant is stored in the section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NumField {
    InicioAtividade = 1,
    DataSituacao = 2,
    OpcaoSimples = 3,
    ExclusaoSimples = 4,
    OpcaoMei = 5,
    ExclusaoMei = 6,
    CapitalSocial = 7,
}

impl NumField {
    pub const ALL: [NumField; 7] = [
        NumField::InicioAtividade,
        NumField::DataSituacao,
        NumField::OpcaoSimples,
        NumField::ExclusaoSimples,
        NumField::OpcaoMei,
        NumField::ExclusaoMei,
        NumField::CapitalSocial,
    ];

    pub fn name(self) -> &'static str {
        match self {
            NumField::InicioAtividade => "data_inicio_atividade",
            NumField::DataSituacao => "data_situacao_cadastral",
            NumField::OpcaoSimples => "data_opcao_simples",
            NumField::ExclusaoSimples => "data_exclusao_simples",
            NumField::OpcaoMei => "data_opcao_mei",
            NumField::ExclusaoMei => "data_exclusao_mei",
            NumField::CapitalSocial => "capital_social",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == s)
    }

    pub fn is_date(self) -> bool {
        self != NumField::CapitalSocial
    }
}

/// Days since 1900-01-01 plus one (so 0 stays "no value"); `None` for anything that is not
/// a real `YYYYMMDD` date from 1900 on (including `00000000`).
pub fn date_value(s: &str) -> Option<u64> {
    let s = s.trim();
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 8 || digits.len() != s.chars().filter(|c| !matches!(c, '-' | '/')).count() {
        return None;
    }
    let y: i64 = digits[..4].parse().ok()?;
    let m: u32 = digits[4..6].parse().ok()?;
    let d: u32 = digits[6..].parse().ok()?;
    if !(1900..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some((days_from_civil(y, m, d) - days_from_civil(1900, 1, 1)) as u64 + 1)
}

/// Inverse of [`date_value`]: `YYYY-MM-DD`.
pub fn date_string(v: u64) -> String {
    let (y, m, d) = civil_from_days(v as i64 - 1 + days_from_civil(1900, 1, 1));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Year of a [`date_value`] (table lookup for 1900–2199, calendar math beyond).
pub fn date_year(v: u64) -> i64 {
    static YEARS: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    let t = YEARS.get_or_init(|| (0..110_000u64).map(|v| civil_from_days(v as i64 - 1 + days_from_civil(1900, 1, 1)).0 as u16).collect());
    match t.get(v as usize) {
        Some(&y) if v > 0 => i64::from(y),
        _ => civil_from_days(v as i64 - 1 + days_from_civil(1900, 1, 1)).0,
    }
}

/// `1.000,50` / `1000,50` / `1000.50` / `1000` (reais) → cents + 1; `None` when not a number.
pub fn money_value(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (int, frac) = match s.rfind([',', '.']) {
        Some(i) if s.len() - i - 1 <= 2 && s[i + 1..].chars().all(|c| c.is_ascii_digit()) => (&s[..i], &s[i + 1..]),
        _ => (s, ""),
    };
    let int: String = int.chars().filter(|c| !matches!(c, '.' | ',')).collect();
    if int.is_empty() || !int.chars().all(|c| c.is_ascii_digit()) || int.len() > 16 {
        return None;
    }
    let frac = format!("{frac:0<2}");
    let cents = int.parse::<u64>().ok()?.checked_mul(100)?.checked_add(frac.parse::<u64>().ok()?)?;
    cents.checked_add(1)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Values of every [`NumField`] (in `NumField::ALL` order) for each search document of a
/// company: one entry per establishment, or one for a company without establishments.
/// Company-level values (Simples/MEI dates, capital) repeat on every document.
pub fn company_values(c: &opencnpj_core::Company) -> Vec<[u64; 7]> {
    use opencnpj_core::schema::col;
    let s = c.simples.first();
    let sv = |i: usize| s.and_then(|r| date_value(&r[i])).unwrap_or(0);
    let capital = c.empresas.first().and_then(|e| money_value(&e[col::emp::CAPITAL])).unwrap_or(0);
    let company = [
        sv(col::sim::DATA_OPCAO_SIMPLES),
        sv(col::sim::DATA_EXCLUSAO_SIMPLES),
        sv(col::sim::DATA_OPCAO_MEI),
        sv(col::sim::DATA_EXCLUSAO_MEI),
    ];
    let row = |inicio: u64, situacao: u64| [inicio, situacao, company[0], company[1], company[2], company[3], capital];
    if c.estabelecimentos.is_empty() {
        return vec![row(0, 0)];
    }
    c.estabelecimentos
        .iter()
        .map(|e| row(date_value(&e[col::est::DATA_INICIO]).unwrap_or(0), date_value(&e[col::est::DATA_SITUACAO]).unwrap_or(0)))
        .collect()
}

// ---------------------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------------------

struct ColumnWriter {
    field: NumField,
    pending: Vec<u64>,
    dir: Vec<u8>,
    data: Vec<u8>,
}

impl ColumnWriter {
    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let present = self.pending.iter().filter(|&&v| v != 0);
        let (min, max) = present.fold((u64::MAX, 0), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        let has_missing = self.pending.contains(&0);
        let all_missing = min == u64::MAX;
        let (min, max) = if all_missing { (0, 0) } else { (min, max) };
        let bits = if all_missing { 0 } else { (u64::BITS - (max - min + 1).leading_zeros()) as u8 };
        let rel = self.data.len() as u64;
        let mut packed = vec![0u8; (self.pending.len() * bits as usize).div_ceil(8) + PADDING];
        if bits > 0 {
            for (i, &v) in self.pending.iter().enumerate() {
                let x = if v == 0 { 0 } else { v - min + 1 };
                put_bits(&mut packed, i * bits as usize, bits, x);
            }
        }
        packed.truncate(packed.len() - PADDING);
        self.data.extend_from_slice(&packed);
        self.dir.extend_from_slice(&min.to_le_bytes());
        self.dir.extend_from_slice(&max.to_le_bytes());
        self.dir.extend_from_slice(&rel.to_le_bytes());
        self.dir.push(bits);
        self.dir.push((u8::from(has_missing) * HAS_MISSING) | (u8::from(all_missing) * ALL_MISSING));
        self.dir.extend_from_slice(&[0; 6]);
        self.pending.clear();
    }
}

/// Bits are written little-endian; values up to 64 bits may straddle 9 bytes.
fn put_bits(buf: &mut [u8], bit: usize, bits: u8, v: u64) {
    let at = bit / 8;
    let n = (buf.len() - at).min(16);
    let mut b = [0u8; 16];
    b[..n].copy_from_slice(&buf[at..at + n]);
    let cur = u128::from_le_bytes(b) | (u128::from(v) << (bit % 8));
    debug_assert!(bits == 64 || v >> bits == 0);
    buf[at..at + n].copy_from_slice(&cur.to_le_bytes()[..n]);
}

fn get_bits(buf: &[u8], bit: usize, bits: u8) -> u64 {
    if bits == 0 {
        return 0;
    }
    let at = bit / 8;
    let shift = bit % 8;
    let mut b = [0u8; 16];
    let n = (buf.len().saturating_sub(at)).min(16);
    b[..n].copy_from_slice(&buf[at..at + n]);
    let word = u128::from_le_bytes(b) >> shift;
    (word & ((1u128 << bits) - 1)) as u64
}

/// Builds the section from values pushed in document order.
pub struct NumericBuilder {
    columns: Vec<ColumnWriter>,
    docs: u32,
}

impl NumericBuilder {
    pub fn new(fields: &[NumField]) -> Self {
        Self {
            columns: fields
                .iter()
                .map(|&field| ColumnWriter { field, pending: Vec::with_capacity(BLOCK_DOCS), dir: Vec::new(), data: Vec::new() })
                .collect(),
            docs: 0,
        }
    }

    /// One document: `values[i]` belongs to the i-th field given to `new` (0 = no value).
    pub fn push(&mut self, values: &[u64]) {
        for (c, &v) in self.columns.iter_mut().zip(values) {
            c.pending.push(v);
            if c.pending.len() == BLOCK_DOCS {
                c.flush();
            }
        }
        self.docs += 1;
    }

    pub fn finish(mut self, out: &mut impl Write) -> Result<u64, Error> {
        for c in &mut self.columns {
            c.flush();
        }
        let mut header = Vec::new();
        header.extend_from_slice(&(self.columns.len() as u32).to_le_bytes());
        header.extend_from_slice(&self.docs.to_le_bytes());
        header.extend_from_slice(&(BLOCK_DOCS as u32).to_le_bytes());
        header.extend_from_slice(&[0; 4]);
        let mut off = (HEADER_LEN + self.columns.len() * ENTRY_LEN) as u64;
        for c in &self.columns {
            let n_blocks = (c.dir.len() / DIR_LEN) as u64;
            let data_len = (c.data.len() + PADDING) as u64;
            header.push(c.field as u8);
            header.extend_from_slice(&[0; 7]);
            for v in [n_blocks, off, off + c.dir.len() as u64, data_len] {
                header.extend_from_slice(&v.to_le_bytes());
            }
            off += c.dir.len() as u64 + data_len;
        }
        out.write_all(&header)?;
        for c in &self.columns {
            out.write_all(&c.dir)?;
            out.write_all(&c.data)?;
            out.write_all(&[0; PADDING])?;
        }
        Ok(off)
    }
}

// ---------------------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Block {
    min: u64,
    max: u64,
    rel: usize,
    bits: u8,
    flags: u8,
}

/// One column, borrowing nothing: callers pass the mapped file.
#[derive(Debug, Clone)]
pub struct NumericColumn {
    pub field: NumField,
    doc_count: u32,
    blocks: Vec<Block>,
    /// Absolute byte range of the packed data (padding included).
    data: (usize, usize),
}

/// Inclusive range on stored values; `missing` selects documents without a value instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub lo: u64,
    pub hi: u64,
}

impl NumericColumn {
    /// Bytes on disk (block directory + packed data) and the mean bits per document.
    pub fn stored_bytes(&self) -> (usize, f64) {
        let bytes = self.blocks.len() * DIR_LEN + (self.data.1 - self.data.0);
        let bits: u64 = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let docs = (self.doc_count as usize - i * BLOCK_DOCS).min(BLOCK_DOCS);
                u64::from(b.bits) * docs as u64
            })
            .sum();
        (bytes, bits as f64 / f64::from(self.doc_count.max(1)))
    }

    /// Like [`Self::get`] with a 64-bit read (values up to 56 bits; wider ones fall back).
    fn get_fast(&self, file: &[u8], doc: u32) -> u64 {
        let b = &self.blocks[doc as usize / BLOCK_DOCS];
        if b.flags & ALL_MISSING != 0 || b.bits == 0 {
            return 0;
        }
        if b.bits > 56 {
            return self.get(file, doc);
        }
        let bit = (doc as usize % BLOCK_DOCS) * b.bits as usize;
        let at = self.data.0 + b.rel + bit / 8;
        let w = u64::from_le_bytes(file[at..at + 8].try_into().expect("8"));
        let x = (w >> (bit % 8)) & ((1u64 << b.bits) - 1);
        if x == 0 { 0 } else { b.min + x - 1 }
    }

    /// Stored value (0 = none) of `doc`.
    pub fn get(&self, file: &[u8], doc: u32) -> u64 {
        if doc >= self.doc_count {
            return 0;
        }
        let b = &self.blocks[doc as usize / BLOCK_DOCS];
        if b.flags & ALL_MISSING != 0 {
            return 0;
        }
        let data = &file[self.data.0 + b.rel..self.data.1];
        let x = get_bits(data, (doc as usize % BLOCK_DOCS) * b.bits as usize, b.bits);
        if x == 0 { 0 } else { b.min + x - 1 }
    }

    /// Decodes block `i` into `out` (stored values, 0 = none). Uses 64-bit unaligned reads;
    /// the 8 padding bytes after the data make every read in bounds.
    pub fn decode_block(&self, file: &[u8], i: usize, out: &mut Vec<u64>) {
        let b = &self.blocks[i];
        let docs = (self.doc_count as usize - i * BLOCK_DOCS).min(BLOCK_DOCS);
        out.clear();
        if b.flags & ALL_MISSING != 0 || b.bits == 0 {
            out.resize(docs, 0);
            return;
        }
        let data = &file[self.data.0 + b.rel..self.data.1];
        let bits = b.bits as usize;
        if bits <= 56 {
            let mask = (1u64 << bits) - 1;
            out.extend((0..docs).map(|j| {
                let bit = j * bits;
                let w = u64::from_le_bytes(data[bit / 8..bit / 8 + 8].try_into().expect("8"));
                let x = (w >> (bit % 8)) & mask;
                if x == 0 { 0 } else { b.min + x - 1 }
            }));
        } else {
            out.extend((0..docs).map(|j| {
                let x = get_bits(data, j * bits, b.bits);
                if x == 0 { 0 } else { b.min + x - 1 }
            }));
        }
    }

    /// Documents whose value lies in `range` (`None` = documents without a value),
    /// optionally restricted to `within`. Blocks that cannot match, or that contain no
    /// document of `within`, are skipped without decoding; fully matching blocks are taken
    /// whole.
    pub fn select(&self, file: &[u8], range: Option<Range>, within: Option<&RoaringBitmap>) -> RoaringBitmap {
        let matches = |v: u64| match range {
            Some(r) => v != 0 && v >= r.lo && v <= r.hi,
            None => v == 0,
        };
        let mut out = RoaringBitmap::new();
        let mut vals = Vec::with_capacity(BLOCK_DOCS);
        if let Some(set) = within.filter(|s| s.len() <= u64::from(self.doc_count) / 4) {
            // Candidates given: probe only them. A block with many candidates is decoded
            // once; a block with few is read value by value. Matches are appended in
            // ascending order (Roaring's fast path).
            let mut hits = Vec::new();
            let mut it = set.iter().peekable();
            while let Some(&d) = it.peek() {
                let bi = d as usize / BLOCK_DOCS;
                if bi >= self.blocks.len() {
                    break;
                }
                let end = ((bi + 1) * BLOCK_DOCS) as u32;
                let mut docs: Vec<u32> = Vec::new();
                while let Some(&x) = it.peek() {
                    if x >= end {
                        break;
                    }
                    docs.push(x);
                    it.next();
                }
                match self.classify(bi, range) {
                    1 => {}
                    2 => hits.extend_from_slice(&docs),
                    _ if docs.len() >= 64 => {
                        self.decode_block(file, bi, &mut vals);
                        hits.extend(docs.into_iter().filter(|&x| matches(vals[x as usize % BLOCK_DOCS])));
                    }
                    _ => hits.extend(docs.into_iter().filter(|&x| matches(self.get_fast(file, x)))),
                }
            }
            out.append(hits).expect("ascending");
            return out;
        }
        for (i, b) in self.blocks.iter().enumerate() {
            let start = (i * BLOCK_DOCS) as u32;
            let end = ((i + 1) * BLOCK_DOCS).min(self.doc_count as usize) as u32;
            let all_missing = b.flags & ALL_MISSING != 0;
            let has_missing = b.flags & HAS_MISSING != 0;
            match range {
                Some(r) => {
                    if all_missing || b.max < r.lo || b.min > r.hi {
                        continue;
                    }
                    if !has_missing && b.min >= r.lo && b.max <= r.hi {
                        out.insert_range(start..end);
                        continue;
                    }
                }
                None => {
                    if all_missing {
                        out.insert_range(start..end);
                        continue;
                    }
                    if !has_missing {
                        continue;
                    }
                }
            }
            if within.is_some_and(|s| s.range_cardinality(start..end) == 0) {
                continue;
            }
            self.decode_block(file, i, &mut vals);
            out.append(vals.iter().enumerate().filter(|(_, v)| matches(**v)).map(|(j, _)| start + j as u32)).expect("ascending");
        }
        if let Some(set) = within {
            out &= set;
        }
        out
    }

    /// 0 = must decode, 1 = no document of block `i` matches, 2 = all of them match.
    fn classify(&self, i: usize, range: Option<Range>) -> u8 {
        let b = &self.blocks[i];
        let all_missing = b.flags & ALL_MISSING != 0;
        let has_missing = b.flags & HAS_MISSING != 0;
        match range {
            Some(r) if all_missing || b.max < r.lo || b.min > r.hi => 1,
            Some(r) if !has_missing && b.min >= r.lo && b.max <= r.hi => 2,
            Some(_) => 0,
            None if all_missing => 2,
            None if !has_missing => 1,
            None => 0,
        }
    }

    /// Calls `f(doc, value)` for every document of `set` (ascending), decoding each touched
    /// block once.
    fn for_each(&self, file: &[u8], set: &RoaringBitmap, mut f: impl FnMut(u32, u64)) {
        let mut vals = Vec::with_capacity(BLOCK_DOCS);
        let mut cur = usize::MAX;
        for d in set.iter() {
            let bi = d as usize / BLOCK_DOCS;
            if bi >= self.blocks.len() {
                break;
            }
            if bi != cur {
                self.decode_block(file, bi, &mut vals);
                cur = bi;
            }
            f(d, vals[d as usize % BLOCK_DOCS]);
        }
    }

    /// Up to `k` documents of `set` ordered by value (documents without a value last),
    /// ties in document order. Blocks are visited from the most promising min/max bound and
    /// the scan stops once no remaining block can beat the current k-th value.
    pub fn top_k(&self, file: &[u8], set: &RoaringBitmap, k: usize, desc: bool) -> Vec<u32> {
        if k == 0 {
            return Vec::new();
        }
        // Sort key: smaller is better; documents without a value get u64::MAX.
        let key = |x: u64| match (x, desc) {
            (0, _) => u64::MAX,
            (x, true) => u64::MAX - 1 - x,
            (x, false) => x,
        };
        let mut order: Vec<(u64, usize)> = Vec::new();
        for (i, b) in self.blocks.iter().enumerate() {
            let start = (i * BLOCK_DOCS) as u32;
            let end = ((i + 1) * BLOCK_DOCS).min(self.doc_count as usize) as u32;
            if b.flags & ALL_MISSING != 0 || set.range_cardinality(start..end) == 0 {
                continue;
            }
            order.push((key(if desc { b.max } else { b.min }), i));
        }
        order.sort_unstable();
        let mut heap: std::collections::BinaryHeap<(u64, u32)> = std::collections::BinaryHeap::with_capacity(k + 1);
        let mut vals = Vec::with_capacity(BLOCK_DOCS);
        for (bound, i) in order {
            if heap.len() == k && heap.peek().is_some_and(|w| w.0 < bound) {
                break;
            }
            let start = (i * BLOCK_DOCS) as u32;
            self.decode_block(file, i, &mut vals);
            for (j, &v) in vals.iter().enumerate() {
                if v == 0 {
                    continue;
                }
                let item = (key(v), start + j as u32);
                // Membership is checked only for values that would enter the heap.
                let better = heap.len() < k || heap.peek().is_some_and(|w| item < *w);
                if better && set.contains(item.1) {
                    if heap.len() == k {
                        heap.pop();
                    }
                    heap.push(item);
                }
            }
        }
        let mut out: Vec<(u64, u32)> = heap.into_vec();
        out.sort_unstable();
        let mut docs: Vec<u32> = out.into_iter().map(|(_, d)| d).collect();
        if docs.len() < k {
            // Documents without a value, in document order.
            let have: RoaringBitmap = docs.iter().copied().collect();
            let missing = self.select(file, None, Some(set)) - have;
            docs.extend(missing.iter().take(k - docs.len()));
        }
        docs
    }

    /// Value histogram over `set` (all documents when `None`) with `bucket(value) -> key`
    /// (values > 0 only).
    pub fn histogram(&self, file: &[u8], set: Option<&RoaringBitmap>, bucket: impl Fn(u64) -> i64) -> Vec<(i64, u64)> {
        // Run-length accumulation: neighbouring documents usually share a bucket.
        let mut h = std::collections::BTreeMap::new();
        let mut run: Option<(i64, u64)> = None;
        let mut add = |v: u64| {
            if v == 0 {
                return;
            }
            let k = bucket(v);
            match &mut run {
                Some((rk, n)) if *rk == k => *n += 1,
                _ => {
                    if let Some((rk, n)) = run.replace((k, 1)) {
                        *h.entry(rk).or_insert(0u64) += n;
                    }
                }
            }
        };
        match set {
            Some(s) => self.for_each(file, s, |_, v| add(v)),
            None => {
                let mut vals = Vec::with_capacity(BLOCK_DOCS);
                for i in 0..self.blocks.len() {
                    self.decode_block(file, i, &mut vals);
                    vals.iter().for_each(|&v| add(v));
                }
            }
        }
        if let Some((rk, n)) = run {
            *h.entry(rk).or_insert(0u64) += n;
        }
        h.into_iter().collect()
    }
}

pub fn parse(file: &[u8], start: usize, end: usize, doc_count: u32) -> Result<Vec<NumericColumn>, Error> {
    let s = file.get(start..end).ok_or(Error::Corrupt("numeric section bounds"))?;
    let h = s.get(..HEADER_LEN).ok_or(Error::Corrupt("numeric header"))?;
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().expect("4"));
    let n = u32_at(h, 0) as usize;
    if n > 32 || u32_at(h, 4) != doc_count || u32_at(h, 8) as usize != BLOCK_DOCS {
        return Err(Error::Corrupt("numeric header"));
    }
    let n_blocks_expected = (doc_count as usize).div_ceil(BLOCK_DOCS);
    let mut out: Vec<NumericColumn> = Vec::with_capacity(n);
    let entries = s.get(HEADER_LEN..HEADER_LEN + n * ENTRY_LEN).ok_or(Error::Corrupt("numeric entries"))?;
    for e in entries.as_chunks::<ENTRY_LEN>().0 {
        let u64_at = |i: usize| usize::try_from(u64::from_le_bytes(e[i..i + 8].try_into().expect("8"))).unwrap_or(usize::MAX);
        let field = NumField::ALL.into_iter().find(|f| *f as u8 == e[0]).ok_or(Error::Corrupt("numeric field"))?;
        let (n_blocks, dir, data, data_len) = (u64_at(8), u64_at(16), u64_at(24), u64_at(32));
        if n_blocks != n_blocks_expected
            || dir.checked_add(n_blocks * DIR_LEN) != Some(data)
            || data.checked_add(data_len).is_none_or(|e| e > s.len())
            || data_len < PADDING
            || out.iter().any(|c| c.field == field)
        {
            return Err(Error::Corrupt("numeric column bounds"));
        }
        let mut blocks = Vec::with_capacity(n_blocks);
        for (i, d) in s[dir..data].as_chunks::<DIR_LEN>().0.iter().enumerate() {
            let u = |j: usize| u64::from_le_bytes(d[j..j + 8].try_into().expect("8"));
            let b = Block { min: u(0), max: u(8), rel: usize::try_from(u(16)).unwrap_or(usize::MAX), bits: d[24], flags: d[25] };
            let docs = (doc_count as usize - i * BLOCK_DOCS).min(BLOCK_DOCS);
            let need = (docs * b.bits as usize).div_ceil(8);
            if b.bits > 64 || b.rel.checked_add(need).is_none_or(|e| e + PADDING > data_len) || b.min > b.max {
                return Err(Error::Corrupt("numeric block"));
            }
            blocks.push(b);
        }
        out.push(NumericColumn { field, doc_count, blocks, data: (start + data, start + data + data_len) });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_money() {
        assert_eq!(date_value("19000101"), Some(1));
        assert_eq!(date_string(date_value("20240229").unwrap()), "2024-02-29");
        assert_eq!(date_value("2024-02-29"), date_value("20240229"));
        assert_eq!(date_year(date_value("20070402").unwrap()), 2007);
        for d in ["19000101", "19991231", "20000101", "20240229", "21991231", "22990101"] {
            assert_eq!(date_year(date_value(d).unwrap()), d[..4].parse::<i64>().unwrap(), "{d}");
        }
        for bad in ["00000000", "20230229", "", "18991231", "2007040", "2007-04-02x"] {
            assert_eq!(date_value(bad), None, "{bad}");
        }
        assert_eq!(money_value("1.000,50"), Some(100_051));
        assert_eq!(money_value("1000,5"), Some(100_051));
        assert_eq!(money_value("1000.50"), Some(100_051));
        assert_eq!(money_value("0,00"), Some(1));
        assert_eq!(money_value("1000"), Some(100_001));
        assert_eq!(money_value("abc"), None);
    }

    fn column(values: &[u64]) -> (Vec<u8>, NumericColumn) {
        let mut b = NumericBuilder::new(&[NumField::InicioAtividade]);
        for &v in values {
            b.push(&[v]);
        }
        let mut buf = vec![0xAA; 3];
        b.finish(&mut buf).unwrap();
        let cols = parse(&buf, 3, buf.len(), values.len() as u32).unwrap();
        (buf, cols.into_iter().next().unwrap())
    }

    #[test]
    fn roundtrip_select_sort_histogram() {
        let values: Vec<u64> = (0..5000u64).map(|i| if i % 7 == 0 { 0 } else { 40_000 + i % 300 }).collect();
        let (file, c) = column(&values);
        for (d, &v) in values.iter().enumerate() {
            assert_eq!(c.get(&file, d as u32), v);
        }
        let r = Range { lo: 40_100, hi: 40_150 };
        let want: Vec<u32> = (0..5000).filter(|&d| (r.lo..=r.hi).contains(&values[d as usize])).collect();
        assert_eq!(c.select(&file, Some(r), None).iter().collect::<Vec<_>>(), want);
        let some: RoaringBitmap = (0..5000).step_by(3).collect();
        assert_eq!(
            c.select(&file, Some(r), Some(&some)).iter().collect::<Vec<_>>(),
            want.iter().copied().filter(|d| d % 3 == 0).collect::<Vec<_>>()
        );
        assert_eq!(c.select(&file, None, None).len(), values.iter().filter(|&&v| v == 0).count() as u64);
        let best = some.iter().map(|d| values[d as usize]).max().unwrap();
        let top = c.top_k(&file, &some, 3, true);
        assert!(top.iter().all(|&d| values[d as usize] == best));
        let low = c.top_k(&file, &some, 1, false);
        assert_eq!(values[low[0] as usize], some.iter().map(|d| values[d as usize]).filter(|&v| v > 0).min().unwrap());
        let h = c.histogram(&file, None, |v| (v / 100) as i64);
        assert_eq!(h.iter().map(|x| x.1).sum::<u64>(), values.iter().filter(|&&v| v != 0).count() as u64);
    }

    #[test]
    fn wide_values_and_bounds() {
        let values = vec![1, u64::MAX - 5, 0, 77];
        let (file, c) = column(&values);
        for (d, &v) in values.iter().enumerate() {
            assert_eq!(c.get(&file, d as u32), v);
        }
        let (all_missing_file, all_missing) = column(&[0; 10]);
        assert_eq!(all_missing.select(&all_missing_file, None, None).len(), 10);
        assert!(parse(&file, 3, file.len() - 1, 4).is_err());
        assert!(parse(&file, 3, file.len(), 5).is_err());
    }
}
