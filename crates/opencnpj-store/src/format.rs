//! `.ocnpj` container layout (all integers little-endian):
//!
//! ```text
//! [Header 64 B] [DATA blocks ...] [other sections ...] [section table] [Footer 40 B]
//! ```
//!
//! * Header: magic `OCNPJ\0v1`, major/minor version, flags, competence (`YYYY-MM`),
//!   xxh3 of the preceding header bytes.
//! * Section table: `count u32` then `count` × `{kind u32, codec u32, offset u64, len u64,
//!   raw_len u64, xxh3 u64}` (40 B each). Every section is checksummed independently.
//! * Footer: `table_offset u64, table_len u64, table_xxh3 u64, file_len u64, magic`.
//!
//! Readers validate every offset/length against the file size before touching data.

use xxhash_rust::xxh3::xxh3_64;

use crate::Error;

pub const MAGIC: [u8; 8] = *b"OCNPJ\0v1";
pub const FOOTER_MAGIC: [u8; 8] = *b"OCNPJEND";
pub const FORMAT_MAJOR: u16 = 1;
/// 1.1 added the optional `docvalues` section.
pub const FORMAT_MINOR: u16 = 2;
pub const HEADER_LEN: usize = 64;
pub const FOOTER_LEN: usize = 40;
pub const SECTION_ENTRY_LEN: usize = 40;
pub const BLOCK_ENTRY_LEN: usize = 40;
pub const MAX_SECTIONS: usize = 64;
/// Upper bound for one decoded block; protects readers from hostile `raw_len` values.
pub const MAX_BLOCK_RAW: usize = 256 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SectionKind {
    Data = 1,
    Manifest = 2,
    BlockDir = 3,
    Dictionaries = 4,
    ZstdDict = 5,
    RefTables = 6,
    Rejects = 7,
    Terms = 8,
    Postings = 9,
    /// Forward columns for `group_by` (format 1.1, optional).
    DocValues = 10,
    /// Numeric columns for range filters, sorting and histograms (format 1.2, optional).
    Numeric = 11,
}

impl SectionKind {
    pub fn from_u32(v: u32) -> Option<Self> {
        use SectionKind::*;
        Some(match v {
            1 => Data,
            2 => Manifest,
            3 => BlockDir,
            4 => Dictionaries,
            5 => ZstdDict,
            6 => RefTables,
            7 => Rejects,
            8 => Terms,
            9 => Postings,
            10 => DocValues,
            11 => Numeric,
            _ => return None,
        })
    }
}

/// Section payload codec (independent of the block codec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SectionCodec {
    Raw = 0,
    Zstd = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionEntry {
    pub kind: SectionKind,
    pub codec: SectionCodec,
    pub offset: u64,
    pub len: u64,
    pub raw_len: u64,
    pub xxh3: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub major: u16,
    pub minor: u16,
    pub flags: u32,
    pub competence: [u8; 8],
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        h[..8].copy_from_slice(&MAGIC);
        h[8..10].copy_from_slice(&self.major.to_le_bytes());
        h[10..12].copy_from_slice(&self.minor.to_le_bytes());
        h[12..16].copy_from_slice(&self.flags.to_le_bytes());
        h[16..24].copy_from_slice(&self.competence);
        let sum = xxh3_64(&h[..56]);
        h[56..].copy_from_slice(&sum.to_le_bytes());
        h
    }

    pub fn decode(b: &[u8]) -> Result<Self, Error> {
        if b.len() < HEADER_LEN || b[..8] != MAGIC {
            return Err(Error::Corrupt("bad magic: not an .ocnpj snapshot"));
        }
        if xxh3_64(&b[..56]).to_le_bytes() != b[56..64] {
            return Err(Error::Corrupt("header checksum mismatch"));
        }
        let major = u16::from_le_bytes([b[8], b[9]]);
        if major != FORMAT_MAJOR {
            return Err(Error::Unsupported(format!("format major version {major}")));
        }
        Ok(Self {
            major,
            minor: u16::from_le_bytes([b[10], b[11]]),
            flags: u32::from_le_bytes(b[12..16].try_into().expect("4")),
            competence: b[16..24].try_into().expect("8"),
        })
    }
}

pub fn encode_sections(entries: &[SectionEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + entries.len() * SECTION_ENTRY_LEN);
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for e in entries {
        out.extend_from_slice(&(e.kind as u32).to_le_bytes());
        out.extend_from_slice(&(e.codec as u32).to_le_bytes());
        out.extend_from_slice(&e.offset.to_le_bytes());
        out.extend_from_slice(&e.len.to_le_bytes());
        out.extend_from_slice(&e.raw_len.to_le_bytes());
        out.extend_from_slice(&e.xxh3.to_le_bytes());
    }
    out
}

/// Unknown kinds are skipped only in files written by a newer minor version (`minor`
/// from the header): a minor version may add optional sections, never required ones.
pub fn decode_sections(b: &[u8], file_len: u64, minor: u16) -> Result<Vec<SectionEntry>, Error> {
    let count = u32::from_le_bytes(b.get(..4).ok_or(Error::Corrupt("section table"))?.try_into().expect("4")) as usize;
    if count > MAX_SECTIONS || b.len() != 4 + count * SECTION_ENTRY_LEN {
        return Err(Error::Corrupt("section table size"));
    }
    let mut out = Vec::with_capacity(count);
    for chunk in b[4..].as_chunks::<SECTION_ENTRY_LEN>().0 {
        let u32_at = |i: usize| u32::from_le_bytes(chunk[i..i + 4].try_into().expect("4"));
        let u64_at = |i: usize| u64::from_le_bytes(chunk[i..i + 8].try_into().expect("8"));
        let (offset, len) = (u64_at(8), u64_at(16));
        let end = offset.checked_add(len).ok_or(Error::Corrupt("section overflow"))?;
        if offset < HEADER_LEN as u64 || end > file_len {
            return Err(Error::Corrupt("section out of bounds"));
        }
        let Some(kind) = SectionKind::from_u32(u32_at(0)) else {
            if minor > FORMAT_MINOR {
                continue;
            }
            return Err(Error::Corrupt("unknown section kind"));
        };
        let codec = match u32_at(4) {
            0 => SectionCodec::Raw,
            1 => SectionCodec::Zstd,
            _ => return Err(Error::Corrupt("unknown section codec")),
        };
        let e = SectionEntry { kind, codec, offset, len, raw_len: u64_at(24), xxh3: u64_at(32) };
        if out.iter().any(|o: &SectionEntry| o.kind == kind) {
            return Err(Error::Corrupt("duplicate section"));
        }
        out.push(e);
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footer {
    pub table_offset: u64,
    pub table_len: u64,
    pub table_xxh3: u64,
    pub file_len: u64,
}

impl Footer {
    pub fn encode(&self) -> [u8; FOOTER_LEN] {
        let mut f = [0u8; FOOTER_LEN];
        f[..8].copy_from_slice(&self.table_offset.to_le_bytes());
        f[8..16].copy_from_slice(&self.table_len.to_le_bytes());
        f[16..24].copy_from_slice(&self.table_xxh3.to_le_bytes());
        f[24..32].copy_from_slice(&self.file_len.to_le_bytes());
        f[32..].copy_from_slice(&FOOTER_MAGIC);
        f
    }

    pub fn decode(b: &[u8]) -> Result<Self, Error> {
        if b.len() != FOOTER_LEN || b[32..] != FOOTER_MAGIC {
            return Err(Error::Corrupt("missing footer: truncated snapshot?"));
        }
        let u = |i: usize| u64::from_le_bytes(b[i..i + 8].try_into().expect("8"));
        Ok(Self { table_offset: u(0), table_len: u(8), table_xxh3: u(16), file_len: u(24) })
    }
}

/// One entry of the block directory. Blocks are sorted by `first_key` and `first_doc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockEntry {
    pub first_key: [u8; 8],
    pub offset: u64,
    pub clen: u32,
    pub rlen: u32,
    pub first_doc: u32,
    pub companies: u32,
    pub xxh3: u64,
}

impl BlockEntry {
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.first_key);
        out.extend_from_slice(&self.offset.to_le_bytes());
        out.extend_from_slice(&self.clen.to_le_bytes());
        out.extend_from_slice(&self.rlen.to_le_bytes());
        out.extend_from_slice(&self.first_doc.to_le_bytes());
        out.extend_from_slice(&self.companies.to_le_bytes());
        out.extend_from_slice(&self.xxh3.to_le_bytes());
    }

    pub fn decode_all(b: &[u8]) -> Result<Vec<Self>, Error> {
        if !b.len().is_multiple_of(BLOCK_ENTRY_LEN) {
            return Err(Error::Corrupt("block directory size"));
        }
        Ok(b.as_chunks::<BLOCK_ENTRY_LEN>()
            .0
            .iter()
            .map(|c| {
                let u32_at = |i: usize| u32::from_le_bytes(c[i..i + 4].try_into().expect("4"));
                let u64_at = |i: usize| u64::from_le_bytes(c[i..i + 8].try_into().expect("8"));
                Self {
                    first_key: c[..8].try_into().expect("8"),
                    offset: u64_at(8),
                    clen: u32_at(16),
                    rlen: u32_at(20),
                    first_doc: u32_at(24),
                    companies: u32_at(28),
                    xxh3: u64_at(32),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip_and_tamper() {
        let h = Header { major: FORMAT_MAJOR, minor: 0, flags: 3, competence: *b"2026-10\0" };
        let mut b = h.encode();
        assert_eq!(Header::decode(&b).unwrap(), h);
        b[20] ^= 1;
        assert!(Header::decode(&b).is_err());
    }

    #[test]
    fn sections_bounds() {
        let e = SectionEntry { kind: SectionKind::Manifest, codec: SectionCodec::Raw, offset: 64, len: 10, raw_len: 10, xxh3: 0 };
        let b = encode_sections(&[e]);
        assert_eq!(decode_sections(&b, 100, FORMAT_MINOR).unwrap(), vec![e]);
        assert!(decode_sections(&b, 70, FORMAT_MINOR).is_err());
        assert!(decode_sections(&b[..b.len() - 1], 100, FORMAT_MINOR).is_err());
        assert!(decode_sections(&encode_sections(&[e, e]), 100, FORMAT_MINOR).is_err());
    }

    #[test]
    fn unknown_kinds_only_from_newer_minors() {
        let e = SectionEntry { kind: SectionKind::Manifest, codec: SectionCodec::Raw, offset: 64, len: 10, raw_len: 10, xxh3: 0 };
        let mut b = encode_sections(&[e, e]);
        b[4 + SECTION_ENTRY_LEN..8 + SECTION_ENTRY_LEN].copy_from_slice(&99u32.to_le_bytes());
        assert!(decode_sections(&b, 100, FORMAT_MINOR).is_err());
        assert_eq!(decode_sections(&b, 100, FORMAT_MINOR + 1).unwrap(), vec![e]);
    }
}
