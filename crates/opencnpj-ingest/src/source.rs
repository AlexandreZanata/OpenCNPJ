//! Source discovery and safe streaming of the CSV member inside each official ZIP.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use opencnpj_core::{RefTable, Table};
use sha2::{Digest, Sha256};

use crate::Error;

/// Hard limits against ZIP bombs and hostile archives.
pub const MAX_MEMBER_BYTES: u64 = 64 << 30;
pub const MAX_RATIO: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceKind {
    Fact(Table),
    Ref(RefTable),
}

#[derive(Debug, Clone)]
pub struct Source {
    pub kind: SourceKind,
    pub path: PathBuf,
    pub file_name: String,
    pub bytes: u64,
}

/// Lists recognized ZIPs in `dir` (sorted, deterministic). Unknown files are ignored.
pub fn discover(dir: &Path) -> Result<Vec<Source>, Error> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let ft = e.file_type()?;
        if !ft.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".zip") else { continue };
        let kind = Table::ALL
            .iter()
            .find(|t| stem.strip_prefix(t.zip_prefix()).is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit())))
            .map(|t| SourceKind::Fact(*t))
            .or_else(|| RefTable::ALL.iter().find(|r| r.zip_name() == stem).map(|r| SourceKind::Ref(*r)));
        if let Some(kind) = kind {
            out.push(Source { kind, path: e.path(), file_name: name, bytes: e.metadata()?.len() });
        }
    }
    out.sort_by_key(|a| (a.kind, natural_key(&a.file_name)));
    Ok(out)
}

fn natural_key(name: &str) -> (usize, String) {
    (name.len(), name.to_string())
}

/// Opens the single CSV member of a ZIP, enforcing size limits while reading.
pub struct Member {
    pub name: String,
    pub declared_size: u64,
    archive: zip::ZipArchive<BufReader<File>>,
}

impl Member {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let file = File::open(path)?;
        let zip_len = file.metadata()?.len();
        let mut archive =
            zip::ZipArchive::new(BufReader::with_capacity(1 << 20, file)).map_err(|e| Error::Source(format!("{}: {e}", path.display())))?;
        let files: Vec<usize> = (0..archive.len()).filter(|&i| archive.by_index_raw(i).map(|f| f.is_file()).unwrap_or(false)).collect();
        if files.len() != 1 {
            return Err(Error::Source(format!("{}: expected exactly one file member, found {}", path.display(), files.len())));
        }
        let (name, declared_size) = {
            let f = archive.by_index_raw(files[0]).map_err(|e| Error::Source(e.to_string()))?;
            // The member is streamed, never extracted, but reject traversal-shaped names anyway.
            if f.enclosed_name().is_none() {
                return Err(Error::Source(format!("{}: unsafe member name", path.display())));
            }
            (f.name().to_string(), f.size())
        };
        if declared_size > MAX_MEMBER_BYTES || declared_size > zip_len.saturating_mul(MAX_RATIO).max(1 << 20) {
            return Err(Error::Source(format!("{}: member size {declared_size} exceeds limits", path.display())));
        }
        if files[0] != 0 {
            return Err(Error::Source(format!("{}: unexpected archive layout", path.display())));
        }
        Ok(Self { name, declared_size, archive })
    }

    /// Streams the decompressed member. Reading past the declared size is an error.
    pub fn reader(&mut self) -> Result<impl Read + '_, Error> {
        let limit = self.declared_size;
        let f = self.archive.by_index(0).map_err(|e| Error::Source(e.to_string()))?;
        Ok(Limited { inner: f, left: limit + 1 })
    }

    /// RFB snapshot id in the member name, e.g. `D61010`.
    pub fn snapshot_id(&self) -> Option<String> {
        self.name.split('.').find(|p| p.len() == 6 && p.starts_with('D') && p[1..].chars().all(|c| c.is_ascii_digit())).map(str::to_string)
    }
}

struct Limited<R> {
    inner: R,
    left: u64,
}

impl<R: Read> Read for Limited<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.left = self.left.saturating_sub(n as u64);
        if self.left == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "zip member larger than declared size"));
        }
        Ok(n)
    }
}

/// SHA-256 of a file, streamed.
pub fn sha256_file(path: &Path) -> Result<String, Error> {
    let mut f = BufReader::with_capacity(1 << 20, File::open(path)?);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Reads a reference table (`codigo;descricao`) completely; these are tiny.
pub fn read_ref_table(path: &Path) -> Result<(Vec<[String; 2]>, Vec<crate::parse::RejectedLine>), Error> {
    let mut m = Member::open(path)?;
    let mut raw = Vec::new();
    m.reader()?.read_to_end(&mut raw)?;
    let mut rows = Vec::new();
    let mut rejects = Vec::new();
    crate::parse::for_each_record(&raw, 2, 0, |rec| match rec {
        Ok((_, _, fields)) => {
            let mut a = String::new();
            let mut b = String::new();
            opencnpj_core::text::latin1_to_string(fields[0], &mut a);
            opencnpj_core::text::latin1_to_string(fields[1], &mut b);
            rows.push([a, b]);
        }
        Err(r) => rejects.push(r),
    });
    Ok((rows, rejects))
}
