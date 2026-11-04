//! Merges per-bucket term runs into the final FST + postings sections.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::Error;
use crate::postings::{RunReader, encode_postings};

#[derive(Debug, Default, Clone, Copy)]
pub struct IndexStats {
    pub terms: u64,
    pub postings_bytes: u64,
    pub fst_bytes: u64,
}

/// `runs` must be ordered by `base` (document offset of their bucket). `on_term` sees every
/// term in byte order with its global document ids (used to fill forward columns).
pub fn merge_runs(
    runs: &[(PathBuf, u32)],
    fst_path: &Path,
    postings_path: &Path,
    mut on_term: impl FnMut(&[u8], &[u32]) -> Result<(), Error>,
) -> Result<IndexStats, Error> {
    let mut readers = Vec::with_capacity(runs.len());
    for (path, base) in runs {
        readers.push(RunReader::new(File::open(path)?, *base)?);
    }
    let mut heads: Vec<(Vec<u8>, Vec<u32>)> = vec![(Vec::new(), Vec::new()); readers.len()];
    let mut heap = BinaryHeap::new();
    for (i, r) in readers.iter_mut().enumerate() {
        let (t, d) = &mut heads[i];
        if r.next_term(t, d)? {
            heap.push(Reverse((t.clone(), i)));
        }
    }
    let mut fst =
        fst::MapBuilder::new(BufWriter::with_capacity(1 << 20, File::create(fst_path)?)).map_err(|e| Error::Invalid(e.to_string()))?;
    let mut postings = BufWriter::with_capacity(1 << 20, File::create(postings_path)?);
    let mut stats = IndexStats::default();
    let mut docs: Vec<u32> = Vec::new();
    let mut buf = Vec::new();
    while let Some(Reverse((term, _))) = heap.peek().cloned() {
        docs.clear();
        while let Some(Reverse((t, i))) = heap.peek() {
            if *t != term {
                break;
            }
            let i = *i;
            heap.pop();
            let (ht, hd) = &mut heads[i];
            if docs.last().is_some_and(|l| hd.first().is_some_and(|f| f <= l)) {
                return Err(Error::Corrupt("runs are not ordered by document base"));
            }
            docs.extend_from_slice(hd);
            if readers[i].next_term(ht, hd)? {
                heap.push(Reverse((ht.clone(), i)));
            }
        }
        on_term(&term, &docs)?;
        buf.clear();
        encode_postings(&docs, &mut buf)?;
        fst.insert(&term, stats.postings_bytes).map_err(|e| Error::Invalid(e.to_string()))?;
        postings.write_all(&buf)?;
        stats.postings_bytes += buf.len() as u64;
        stats.terms += 1;
    }
    let mut w = fst.into_inner().map_err(|e| Error::Invalid(e.to_string()))?;
    w.flush()?;
    postings.flush()?;
    stats.fst_bytes = std::fs::metadata(fst_path)?.len();
    Ok(stats)
}
