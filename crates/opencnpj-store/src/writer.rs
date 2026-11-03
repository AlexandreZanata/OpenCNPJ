//! Sequential snapshot writer. The file is written to a temporary sibling and only renamed
//! into place after it is complete and synced, so readers never observe a partial snapshot.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

use crate::Error;
use crate::format::*;

/// Block metadata produced by the encoder before the block is placed in the file.
#[derive(Debug, Clone, Copy)]
pub struct BlockMeta {
    pub first_key: [u8; 8],
    pub clen: u32,
    pub rlen: u32,
    pub first_doc: u32,
    pub companies: u32,
    pub xxh3: u64,
}

struct HashingWriter {
    inner: BufWriter<File>,
    sha: Sha256,
    pos: u64,
}

impl Write for HashingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.sha.update(&buf[..n]);
        self.pos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub struct SnapshotWriter {
    out: HashingWriter,
    tmp: TmpGuard,
    dest: PathBuf,
    sections: Vec<SectionEntry>,
    blocks: Vec<BlockEntry>,
    data_start: u64,
    data_hash: Xxh3,
    data_done: bool,
}

/// Result of a completed write.
#[derive(Debug, Clone)]
pub struct Written {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub sections: Vec<SectionEntry>,
}

impl SnapshotWriter {
    pub fn create(dest: &Path, competence: &str, flags: u32) -> Result<Self, Error> {
        let mut comp = [0u8; 8];
        let cb = competence.as_bytes();
        if cb.len() > 8 {
            return Err(Error::Invalid(format!("competence too long: {competence}")));
        }
        comp[..cb.len()].copy_from_slice(cb);
        let tmp = dest.with_extension(format!("ocnpj.tmp-{}", std::process::id()));
        let file = File::create(&tmp)?;
        let mut out = HashingWriter { inner: BufWriter::with_capacity(1 << 20, file), sha: Sha256::new(), pos: 0 };
        out.write_all(&Header { major: FORMAT_MAJOR, minor: FORMAT_MINOR, flags, competence: comp }.encode())?;
        Ok(Self {
            data_start: out.pos,
            out,
            tmp: TmpGuard { path: tmp, armed: true },
            dest: dest.to_path_buf(),
            sections: Vec::new(),
            blocks: Vec::new(),
            data_hash: Xxh3::new(),
            data_done: false,
        })
    }

    pub fn blocks(&self) -> &[BlockEntry] {
        &self.blocks
    }

    /// Appends one compressed block; `doc_base` shifts the segment-local doc id.
    pub fn append_block(&mut self, meta: &BlockMeta, bytes: &[u8], doc_base: u32) -> Result<(), Error> {
        debug_assert!(!self.data_done);
        if bytes.len() != meta.clen as usize {
            return Err(Error::Invalid("block length does not match metadata".into()));
        }
        if let Some(last) = self.blocks.last()
            && meta.first_key <= last.first_key {
                return Err(Error::Invalid("blocks must be appended in ascending key order".into()));
            }
        let first_doc = doc_base.checked_add(meta.first_doc).ok_or_else(|| Error::Invalid("more than 2^32 documents".into()))?;
        self.blocks.push(BlockEntry {
            first_key: meta.first_key,
            offset: self.out.pos,
            clen: meta.clen,
            rlen: meta.rlen,
            first_doc,
            companies: meta.companies,
            xxh3: meta.xxh3,
        });
        self.data_hash.update(bytes);
        self.out.write_all(bytes)?;
        Ok(())
    }

    /// Copies a segment file (concatenated compressed blocks) described by `metas`.
    pub fn append_segment(&mut self, segment: &Path, metas: &[BlockMeta], doc_base: u32) -> Result<(), Error> {
        let mut f = std::io::BufReader::with_capacity(1 << 20, File::open(segment)?);
        let mut buf = Vec::new();
        for m in metas {
            buf.resize(m.clen as usize, 0);
            f.read_exact(&mut buf)?;
            if xxh3_64(&buf) != m.xxh3 {
                return Err(Error::Corrupt("segment block checksum mismatch"));
            }
            self.append_block(m, &buf, doc_base)?;
        }
        if f.read(&mut [0u8])? != 0 {
            return Err(Error::Corrupt("segment has trailing bytes"));
        }
        Ok(())
    }

    fn end_data(&mut self) {
        if !self.data_done {
            let len = self.out.pos - self.data_start;
            self.sections.push(SectionEntry {
                kind: SectionKind::Data,
                codec: SectionCodec::Raw,
                offset: self.data_start,
                len,
                raw_len: len,
                xxh3: self.data_hash.digest(),
            });
            self.data_done = true;
        }
    }

    pub fn section(&mut self, kind: SectionKind, raw: &[u8], compress: bool) -> Result<(), Error> {
        self.end_data();
        let (codec, bytes) = if compress {
            (SectionCodec::Zstd, std::borrow::Cow::Owned(zstd::bulk::compress(raw, 19)?))
        } else {
            (SectionCodec::Raw, std::borrow::Cow::Borrowed(raw))
        };
        let offset = self.out.pos;
        self.out.write_all(&bytes)?;
        self.sections.push(SectionEntry { kind, codec, offset, len: bytes.len() as u64, raw_len: raw.len() as u64, xxh3: xxh3_64(&bytes) });
        Ok(())
    }

    /// Streams a large uncompressed section from a file.
    pub fn section_from_file(&mut self, kind: SectionKind, path: &Path) -> Result<(), Error> {
        self.end_data();
        let mut f = std::io::BufReader::with_capacity(1 << 20, File::open(path)?);
        let offset = self.out.pos;
        let mut h = Xxh3::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            self.out.write_all(&buf[..n])?;
        }
        let len = self.out.pos - offset;
        self.sections.push(SectionEntry { kind, codec: SectionCodec::Raw, offset, len, raw_len: len, xxh3: h.digest() });
        Ok(())
    }

    pub fn finish(mut self) -> Result<Written, Error> {
        self.end_data();
        let mut dir = Vec::with_capacity(self.blocks.len() * BLOCK_ENTRY_LEN);
        for b in &self.blocks {
            b.encode_into(&mut dir);
        }
        self.section(SectionKind::BlockDir, &dir, false)?;
        let table = encode_sections(&self.sections);
        let table_offset = self.out.pos;
        self.out.write_all(&table)?;
        let file_len = self.out.pos + FOOTER_LEN as u64;
        let footer = Footer { table_offset, table_len: table.len() as u64, table_xxh3: xxh3_64(&table), file_len };
        self.out.write_all(&footer.encode())?;
        self.out.flush()?;
        let sha256 = hex::encode(self.out.sha.clone().finalize());
        let file = self.out.inner.into_inner().map_err(|e| Error::Io(e.into_error()))?;
        file.sync_all()?;
        drop(file);
        fs::rename(&self.tmp.path, &self.dest)?;
        self.tmp.armed = false;
        if let Some(parent) = self.dest.parent()
            && let Ok(d) = File::open(parent) {
                let _ = d.sync_all();
            }
        Ok(Written { path: self.dest, bytes: file_len, sha256, sections: self.sections })
    }
}

/// Removes the temporary file unless the snapshot was published.
struct TmpGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for TmpGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}
