//! Phase A: stream every fact-table ZIP, parse records in parallel and spill them into
//! key-range buckets (first three bytes of `cnpj_basico`), so later phases can sort and join
//! one bucket at a time with bounded memory.
//!
//! Pipeline: reader threads (inflate + cut chunks at record boundaries) → bounded channel
//! (backpressure) → parser workers → per-worker buffers → zstd-1 frames appended to bucket
//! files. Dictionary candidates are counted exactly (no sampling) so the result does not
//! depend on scheduling.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossbeam_channel::bounded;
use opencnpj_core::varint;
use opencnpj_core::{Kind, Table};

use crate::Error;
use crate::parse::{self, RejectedLine};
use crate::source::{Member, Source, SourceKind};

pub const CHUNK_BYTES: usize = 4 << 20;
const BUFFER_BYTES: usize = 48 << 10;
const LOCAL_DICT_FLUSH: usize = 100_000;

pub fn bucket_of(key: &[u8]) -> u32 {
    (u32::from(key[0]) << 16) | (u32::from(key[1]) << 8) | u32::from(key[2])
}

/// Dictionary counts per table and row column (only for `Kind::Dict` columns).
pub type DictCounts = Vec<Vec<HashMap<Vec<u8>, u64>>>;

fn empty_counts() -> DictCounts {
    Table::ALL.iter().map(|t| t.row_columns().iter().map(|_| HashMap::new()).collect()).collect()
}

#[derive(Debug, Clone, Default)]
pub struct SourceStats {
    pub member: Option<String>,
    pub member_bytes: u64,
    pub rows: u64,
    pub rejected: u64,
    pub snapshot_id: Option<String>,
}

pub struct PartitionOutput {
    /// Bucket id → which tables have a file.
    pub buckets: std::collections::BTreeMap<u32, [bool; 4]>,
    pub dicts: DictCounts,
    pub sources: Vec<SourceStats>,
    pub rejects: Vec<(usize, RejectedLine)>,
    pub bytes_in: u64,
    pub elapsed_s: f64,
}

pub fn bucket_path(tmp: &Path, table: Table, bucket: u32) -> PathBuf {
    tmp.join(table.name()).join(format!("{bucket:06x}.rows"))
}

type SharedFile = Arc<Mutex<File>>;

struct Chunk {
    src: usize,
    table: Table,
    first_line: u64,
    data: Vec<u8>,
}

struct Shared {
    tmp: PathBuf,
    files: Mutex<HashMap<(Table, u32), SharedFile>>,
    dicts: Mutex<DictCounts>,
    rows: Vec<AtomicU64>,
    rejected: Vec<AtomicU64>,
    rejects: Mutex<Vec<(usize, RejectedLine)>>,
    bytes_in: AtomicU64,
}

impl Shared {
    fn file(&self, table: Table, bucket: u32) -> Result<SharedFile, Error> {
        let mut files = self.files.lock().expect("lock");
        if let Some(f) = files.get(&(table, bucket)) {
            return Ok(f.clone());
        }
        let f = Arc::new(Mutex::new(OpenOptions::new().create(true).append(true).open(bucket_path(&self.tmp, table, bucket))?));
        files.insert((table, bucket), f.clone());
        Ok(f)
    }
}

/// Writes a frame: `u32 compressed_len, u32 raw_len, zstd bytes`.
fn write_frame(f: &Mutex<File>, raw: &[u8]) -> Result<(), Error> {
    let c = zstd::bulk::compress(raw, 1)?;
    let mut frame = Vec::with_capacity(c.len() + 8);
    frame.extend_from_slice(&(c.len() as u32).to_le_bytes());
    frame.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    frame.extend_from_slice(&c);
    f.lock().expect("lock").write_all(&frame)?;
    Ok(())
}

/// Reads every frame of a bucket file into one buffer.
pub fn read_frames(path: &Path) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let hdr = bytes.get(i..i + 8).ok_or_else(|| Error::Source("truncated temp frame".into()))?;
        let clen = u32::from_le_bytes(hdr[..4].try_into().expect("4")) as usize;
        let rlen = u32::from_le_bytes(hdr[4..].try_into().expect("4")) as usize;
        let data = bytes.get(i + 8..i + 8 + clen).ok_or_else(|| Error::Source("truncated temp frame".into()))?;
        let d = zstd::bulk::decompress(data, rlen)?;
        if d.len() != rlen {
            return Err(Error::Source("temp frame length mismatch".into()));
        }
        out.extend_from_slice(&d);
        i += 8 + clen;
    }
    Ok(out)
}

struct Worker<'a> {
    shared: &'a Shared,
    buffers: HashMap<(Table, u32), Vec<u8>>,
    dicts: DictCounts,
    dict_entries: usize,
}

impl Worker<'_> {
    fn process(&mut self, chunk: &Chunk) -> Result<(), Error> {
        let table = chunk.table;
        let ncols = table.columns().len();
        let dict_cols: Vec<usize> = table.row_columns().iter().enumerate().filter(|(_, c)| c.kind == Kind::Dict).map(|(i, _)| i).collect();
        let mut rows = 0u64;
        let mut err: Option<Error> = None;
        parse::for_each_record(&chunk.data, ncols, chunk.first_line, |rec| {
            if err.is_some() {
                return;
            }
            match rec {
                Ok((line, raw, fields)) => {
                    let key = fields[0];
                    if key.len() != 8 || !key.is_ascii() {
                        self.reject(chunk.src, RejectedLine { line, reason: "cnpj_basico is not 8 ASCII characters", raw: raw.to_vec() });
                        return;
                    }
                    rows += 1;
                    let buf = self.buffers.entry((table, bucket_of(key))).or_insert_with(|| Vec::with_capacity(BUFFER_BYTES + 1024));
                    varint::put_lp(buf, key);
                    varint::put(buf, chunk.src as u64);
                    varint::put(buf, line);
                    for f in &fields[1..] {
                        varint::put_lp(buf, f);
                    }
                    if buf.len() >= BUFFER_BYTES {
                        let full = std::mem::take(buf);
                        if let Err(e) = self.shared.file(table, bucket_of(key)).and_then(|f| write_frame(&f, &full)) {
                            err = Some(e);
                        }
                        let mut full = full;
                        full.clear();
                        *self.buffers.get_mut(&(table, bucket_of(key))).expect("buffer") = full;
                    }
                    for &c in &dict_cols {
                        let m = &mut self.dicts[table.index()][c];
                        match m.get_mut(fields[c + 1]) {
                            Some(n) => *n += 1,
                            None => {
                                m.insert(fields[c + 1].to_vec(), 1);
                                self.dict_entries += 1;
                            }
                        }
                    }
                }
                Err(r) => self.reject(chunk.src, r),
            }
        });
        if let Some(e) = err {
            return Err(e);
        }
        self.shared.rows[chunk.src].fetch_add(rows, Ordering::Relaxed);
        if self.dict_entries > LOCAL_DICT_FLUSH {
            self.flush_dicts();
        }
        Ok(())
    }

    fn reject(&self, src: usize, r: RejectedLine) {
        self.shared.rejected[src].fetch_add(1, Ordering::Relaxed);
        self.shared.rejects.lock().expect("lock").push((src, r));
    }

    fn flush_dicts(&mut self) {
        let mut g = self.shared.dicts.lock().expect("lock");
        for (t, cols) in self.dicts.iter_mut().enumerate() {
            for (c, m) in cols.iter_mut().enumerate() {
                for (k, v) in m.drain() {
                    *g[t][c].entry(k).or_insert(0) += v;
                }
            }
        }
        self.dict_entries = 0;
    }

    fn finish(mut self) -> Result<(), Error> {
        self.flush_dicts();
        for ((table, bucket), buf) in std::mem::take(&mut self.buffers) {
            if !buf.is_empty() {
                write_frame(&*self.shared.file(table, bucket)?, &buf)?;
            }
        }
        Ok(())
    }
}

/// Inflates one source and sends record-aligned chunks.
fn read_source(
    src: usize,
    source: &Source,
    tx: &crossbeam_channel::Sender<Chunk>,
    stats: &Mutex<Vec<SourceStats>>,
    shared: &Shared,
) -> Result<(), Error> {
    let SourceKind::Fact(table) = source.kind else { return Ok(()) };
    let mut member = Member::open(&source.path)?;
    {
        let mut s = stats.lock().expect("lock");
        s[src].member = Some(member.name.clone());
        s[src].member_bytes = member.declared_size;
        s[src].snapshot_id = member.snapshot_id();
    }
    let mut r = member.reader()?;
    let mut pending: Vec<u8> = Vec::with_capacity(CHUNK_BYTES * 2);
    let mut line = 1u64;
    let mut eof = false;
    let mut read_more = |pending: &mut Vec<u8>, eof: &mut bool| -> Result<(), Error> {
        let old = pending.len();
        pending.resize(old + (1 << 20), 0);
        let n = r.read(&mut pending[old..])?;
        pending.truncate(old + n);
        *eof = n == 0;
        Ok(())
    };
    loop {
        while pending.len() < CHUNK_BYTES && !eof {
            read_more(&mut pending, &mut eof)?;
        }
        if pending.is_empty() {
            break;
        }
        let cut = if eof {
            pending.len()
        } else {
            match parse::chunk_boundary(&pending, CHUNK_BYTES - (CHUNK_BYTES >> 3)) {
                Some(c) => c,
                None if pending.len() > 64 * CHUNK_BYTES => {
                    return Err(Error::Source("no record boundary within 256 MiB".into()));
                }
                None => {
                    read_more(&mut pending, &mut eof)?;
                    continue;
                }
            }
        };
        let rest = pending.split_off(cut);
        let data = std::mem::replace(&mut pending, rest);
        let lines = memchr::memchr_iter(b'\n', &data).count() as u64;
        shared.bytes_in.fetch_add(data.len() as u64, Ordering::Relaxed);
        tx.send(Chunk { src, table, first_line: line, data }).map_err(|_| Error::Source("parser workers stopped".into()))?;
        line += lines;
    }
    Ok(())
}

pub struct PartitionConfig {
    pub workers: usize,
    pub readers: usize,
}

pub fn run(sources: &[Source], tmp: &Path, cfg: &PartitionConfig, progress: &(dyn Fn(u64) + Sync)) -> Result<PartitionOutput, Error> {
    let t0 = Instant::now();
    for t in Table::ALL {
        std::fs::create_dir_all(tmp.join(t.name()))?;
    }
    let shared = Shared {
        tmp: tmp.to_path_buf(),
        files: Mutex::new(HashMap::new()),
        dicts: Mutex::new(empty_counts()),
        rows: (0..sources.len()).map(|_| AtomicU64::new(0)).collect(),
        rejected: (0..sources.len()).map(|_| AtomicU64::new(0)).collect(),
        rejects: Mutex::new(Vec::new()),
        bytes_in: AtomicU64::new(0),
    };
    let stats = Mutex::new(vec![SourceStats::default(); sources.len()]);
    // Largest sources first so the longest sequential inflate starts immediately.
    let mut order: Vec<usize> = (0..sources.len()).filter(|&i| matches!(sources[i].kind, SourceKind::Fact(_))).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sources[i].bytes));
    let next = AtomicUsize::new(0);
    let (tx, rx) = bounded::<Chunk>(cfg.workers * 2);
    let errors: Mutex<Vec<Error>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..cfg.readers.max(1) {
            let tx = tx.clone();
            let (order, next, stats, shared, errors) = (&order, &next, &stats, &shared, &errors);
            s.spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&src) = order.get(i) else { break };
                    if let Err(e) = read_source(src, &sources[src], &tx, stats, shared) {
                        errors.lock().expect("lock").push(Error::Source(format!("{}: {e}", sources[src].file_name)));
                        break;
                    }
                }
            });
        }
        drop(tx);
        for _ in 0..cfg.workers.max(1) {
            let rx = rx.clone();
            let (shared, errors) = (&shared, &errors);
            s.spawn(move || {
                let mut w = Worker { shared, buffers: HashMap::new(), dicts: empty_counts(), dict_entries: 0 };
                for chunk in rx.iter() {
                    if let Err(e) = w.process(&chunk) {
                        errors.lock().expect("lock").push(e);
                        return;
                    }
                    progress(shared.bytes_in.load(Ordering::Relaxed));
                }
                if let Err(e) = w.finish() {
                    errors.lock().expect("lock").push(e);
                }
            });
        }
    });
    if let Some(e) = errors.into_inner().expect("lock").into_iter().next() {
        return Err(e);
    }
    let mut sources_stats = stats.into_inner().expect("lock");
    for (i, s) in sources_stats.iter_mut().enumerate() {
        s.rows = shared.rows[i].load(Ordering::Relaxed);
        s.rejected = shared.rejected[i].load(Ordering::Relaxed);
    }
    let mut buckets = std::collections::BTreeMap::new();
    for (table, bucket) in shared.files.lock().expect("lock").keys() {
        buckets.entry(*bucket).or_insert([false; 4])[table.index()] = true;
    }
    let mut rejects = shared.rejects.into_inner().expect("lock");
    rejects.sort_by_key(|a| (a.0, a.1.line));
    Ok(PartitionOutput {
        buckets,
        dicts: shared.dicts.into_inner().expect("lock"),
        sources: sources_stats,
        rejects,
        bytes_in: shared.bytes_in.load(Ordering::Relaxed),
        elapsed_s: t0.elapsed().as_secs_f64(),
    })
}
