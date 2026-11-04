//! Forward columns ("doc values"): one bit-packed value id per document for single-valued,
//! high-cardinality filter fields (município, CNAE principal). `group_by` on these fields
//! then iterates the candidate set once instead of intersecting one postings list per value.
//!
//! Section layout (`docvalues`, stored raw so it is read from the mapping in place):
//!
//! ```text
//! u32 n_columns, u32 doc_count
//! n_columns × {field u8, bits u8, reserved [u8; 2], n_values u32,
//!              values_offset u64, values_len u64, ids_offset u64, ids_len u64}   (40 B)
//! values: n_values × (varint len, bytes), strictly ascending (= the field's terms)
//! ids:    doc_count × `bits` bits, little-endian bit order, then 8 zero bytes
//! ```
//!
//! Offsets are relative to the section start. Id `0` means "no value"; id `k ≥ 1` is
//! `values[k - 1]`. The column is derived from the same term stream as the postings, so a
//! document has id `k` exactly when it is in the postings of `values[k - 1]`.

use opencnpj_core::varint::{self, Reader};

use crate::Error;
use crate::postings::Field;

/// Fields stored as doc values (single-valued per document).
pub const FIELDS: [Field; 2] = [Field::Municipio, Field::Cnae];
pub const COLUMN_ENTRY_LEN: usize = 40;
const HEADER_LEN: usize = 8;
const MAX_COLUMNS: usize = 16;
const PADDING: usize = 8;

/// Collects value ids while the index merge streams terms in byte order.
pub struct DocValuesBuilder {
    doc_count: u32,
    columns: Vec<BuilderColumn>,
}

struct BuilderColumn {
    field: Field,
    values: Vec<Vec<u8>>,
    ids: Vec<u16>,
    /// More than `u16::MAX` values: the column is dropped (postings still answer).
    overflow: bool,
}

impl DocValuesBuilder {
    pub fn new(fields: &[Field], doc_count: u32) -> Self {
        let columns = fields
            .iter()
            .map(|&field| BuilderColumn { field, values: Vec::new(), ids: vec![0; doc_count as usize], overflow: false })
            .collect();
        Self { doc_count, columns }
    }

    /// Records one term (`[field][value]`) and its sorted global document ids. Terms must
    /// arrive in ascending byte order.
    pub fn add_term(&mut self, term: &[u8], docs: &[u32]) -> Result<(), Error> {
        let Some((&f, value)) = term.split_first() else { return Ok(()) };
        let Some(c) = self.columns.iter_mut().find(|c| c.field as u8 == f) else { return Ok(()) };
        if c.overflow {
            return Ok(());
        }
        if c.values.last().is_some_and(|last| last.as_slice() >= value) {
            return Err(Error::Invalid("doc values: terms out of order".into()));
        }
        c.values.push(value.to_vec());
        let Ok(id) = u16::try_from(c.values.len()) else {
            c.overflow = true;
            c.ids = Vec::new();
            return Ok(());
        };
        for &d in docs {
            let slot = c.ids.get_mut(d as usize).ok_or_else(|| Error::Invalid("doc values: doc id out of range".into()))?;
            if *slot != 0 {
                return Err(Error::Invalid(format!("doc values: document {d} has two {} values", c.field.name())));
            }
            *slot = id;
        }
        Ok(())
    }

    /// Serializes the section; `None` when no column could be stored.
    pub fn finish(self) -> Option<Vec<u8>> {
        let columns: Vec<BuilderColumn> = self.columns.into_iter().filter(|c| !c.overflow).collect();
        if columns.is_empty() {
            return None;
        }
        let mut header = Vec::new();
        let mut body = Vec::new();
        let body_start = HEADER_LEN + columns.len() * COLUMN_ENTRY_LEN;
        header.extend_from_slice(&(columns.len() as u32).to_le_bytes());
        header.extend_from_slice(&self.doc_count.to_le_bytes());
        for c in &columns {
            let bits = bits_for(c.values.len());
            let values_offset = body_start + body.len();
            for v in &c.values {
                varint::put_lp(&mut body, v);
            }
            let values_len = body_start + body.len() - values_offset;
            let ids_offset = body_start + body.len();
            let packed_len = packed_len(self.doc_count, bits);
            let start = body.len();
            body.resize(start + packed_len + PADDING, 0);
            let packed = &mut body[start..];
            for (d, &id) in c.ids.iter().enumerate() {
                let bit = d * bits as usize;
                let at = bit / 8;
                let word = u64::from_le_bytes(packed[at..at + 8].try_into().expect("8")) | (u64::from(id) << (bit % 8));
                packed[at..at + 8].copy_from_slice(&word.to_le_bytes());
            }
            header.push(c.field as u8);
            header.push(bits);
            header.extend_from_slice(&[0, 0]);
            header.extend_from_slice(&(c.values.len() as u32).to_le_bytes());
            for n in [values_offset, values_len, ids_offset, packed_len + PADDING] {
                header.extend_from_slice(&(n as u64).to_le_bytes());
            }
        }
        header.extend_from_slice(&body);
        Some(header)
    }
}

/// Bits needed for ids `0..=n_values` (at least 1).
fn bits_for(n_values: usize) -> u8 {
    (usize::BITS - n_values.leading_zeros()).max(1) as u8
}

fn packed_len(doc_count: u32, bits: u8) -> usize {
    (doc_count as usize * bits as usize).div_ceil(8)
}

/// Location of one column inside the mapped section.
#[derive(Debug, Clone)]
pub struct DocValuesColumn {
    pub field: Field,
    pub bits: u8,
    pub values: Vec<String>,
    doc_count: u32,
    /// Absolute byte range of the packed ids (padding included) in the file.
    ids: (usize, usize),
}

impl DocValuesColumn {
    /// Value id of `doc` (`file` is the whole mapped file), `None` past the last document.
    pub fn id(&self, file: &[u8], doc: u32) -> Option<u32> {
        if doc >= self.doc_count {
            return None;
        }
        let bit = doc as usize * self.bits as usize;
        let at = self.ids.0 + bit / 8;
        let word = u64::from_le_bytes(file[at..at + 8].try_into().expect("8"));
        Some(((word >> (bit % 8)) & ((1u64 << self.bits) - 1)) as u32)
    }

    /// Counts documents per id over `docs`; index 0 is "no value". Ids above `values.len()`
    /// mean the section is corrupt.
    pub fn count(&self, file: &[u8], docs: impl Iterator<Item = u32>) -> Result<Vec<u64>, Error> {
        let mut counts = vec![0u64; 1 << self.bits];
        let packed = &file[self.ids.0..self.ids.1];
        let (bits, mask) = (self.bits as usize, (1u64 << self.bits) - 1);
        for d in docs {
            if d >= self.doc_count {
                return Err(Error::Corrupt("document out of range"));
            }
            let bit = d as usize * bits;
            let at = bit / 8;
            let word = u64::from_le_bytes(packed[at..at + 8].try_into().expect("8"));
            counts[((word >> (bit % 8)) & mask) as usize] += 1;
        }
        if counts[self.values.len() + 1..].iter().any(|&n| n > 0) {
            return Err(Error::Corrupt("doc value id out of range"));
        }
        counts.truncate(self.values.len() + 1);
        Ok(counts)
    }
}

/// Parses and bounds-checks the section header and value lists (`start..end` is the
/// section's byte range in `file`). Packed ids are validated lazily by [`DocValuesColumn::count`].
pub fn parse(file: &[u8], start: usize, end: usize, doc_count: u32) -> Result<Vec<DocValuesColumn>, Error> {
    let s = file.get(start..end).ok_or(Error::Corrupt("doc values section bounds"))?;
    let h = s.get(..HEADER_LEN).ok_or(Error::Corrupt("doc values header"))?;
    let n = u32::from_le_bytes(h[..4].try_into().expect("4")) as usize;
    let docs = u32::from_le_bytes(h[4..].try_into().expect("4"));
    let entries_end = HEADER_LEN + n * COLUMN_ENTRY_LEN;
    if n > MAX_COLUMNS || s.len() < entries_end || docs != doc_count {
        return Err(Error::Corrupt("doc values header"));
    }
    let mut out: Vec<DocValuesColumn> = Vec::with_capacity(n);
    for e in s[HEADER_LEN..entries_end].as_chunks::<COLUMN_ENTRY_LEN>().0 {
        let u64_at = |i: usize| usize::try_from(u64::from_le_bytes(e[i..i + 8].try_into().expect("8"))).unwrap_or(usize::MAX);
        let field = FIELDS.into_iter().find(|f| *f as u8 == e[0]).ok_or(Error::Corrupt("doc values field"))?;
        let bits = e[1];
        let n_values = u32::from_le_bytes(e[4..8].try_into().expect("4")) as usize;
        if bits != bits_for(n_values) || out.iter().any(|c| c.field == field) {
            return Err(Error::Corrupt("doc values column header"));
        }
        let range = |off: usize, len: usize| off.checked_add(len).filter(|&e| off >= entries_end && e <= s.len()).map(|e| (off, e));
        let (vo, ve) = range(u64_at(8), u64_at(16)).ok_or(Error::Corrupt("doc values bounds"))?;
        let (io, ie) = range(u64_at(24), u64_at(32)).ok_or(Error::Corrupt("doc values bounds"))?;
        if ie - io != packed_len(doc_count, bits) + PADDING {
            return Err(Error::Corrupt("doc values length"));
        }
        let mut r = Reader::new(&s[vo..ve]);
        let mut values: Vec<String> = Vec::with_capacity(n_values.min(ve - vo));
        for _ in 0..n_values {
            let v = String::from_utf8(r.lp_bytes()?.to_vec()).map_err(|_| Error::Corrupt("doc values utf-8"))?;
            if values.last().is_some_and(|l| *l >= v) {
                return Err(Error::Corrupt("doc values order"));
            }
            values.push(v);
        }
        if !r.is_empty() {
            return Err(Error::Corrupt("doc values trailing bytes"));
        }
        out.push(DocValuesColumn { field, bits, values, doc_count, ids: (start + io, start + ie) });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_bounds() {
        let mut b = DocValuesBuilder::new(&FIELDS, 10);
        b.add_term(&Field::Municipio.term(b"7107"), &[0, 3, 9]).unwrap();
        b.add_term(&Field::Municipio.term(b"9701"), &[1]).unwrap();
        b.add_term(&Field::Cnae.term(b"4712100"), &[2, 3]).unwrap();
        b.add_term(&Field::Uf.term(b"SP"), &[0, 1]).unwrap();
        assert!(b.add_term(&Field::Cnae.term(b"0000001"), &[5]).is_err(), "order");
        let bytes = b.finish().unwrap();
        let mut file = vec![0xEE; 7];
        file.extend_from_slice(&bytes);
        let cols = parse(&file, 7, file.len(), 10).unwrap();
        assert_eq!(cols.len(), 2);
        let m = &cols[0];
        assert_eq!((m.field, m.bits, m.values.clone()), (Field::Municipio, 2, vec!["7107".to_string(), "9701".to_string()]));
        assert_eq!((0..10).map(|d| m.id(&file, d).unwrap()).collect::<Vec<_>>(), [1, 2, 0, 1, 0, 0, 0, 0, 0, 1]);
        assert_eq!(m.id(&file, 10), None);
        assert_eq!(m.count(&file, 0..10).unwrap(), [6, 3, 1]);
        assert_eq!(cols[1].count(&file, [2, 3, 4].into_iter()).unwrap(), [1, 2]);
        assert!(parse(&file, 7, file.len(), 11).is_err(), "doc count mismatch");
        assert!(parse(&file, 7, file.len() - 1, 10).is_err(), "truncated");
    }

    #[test]
    fn two_values_for_one_document_is_an_error() {
        let mut b = DocValuesBuilder::new(&FIELDS, 4);
        b.add_term(&Field::Cnae.term(b"1"), &[1]).unwrap();
        assert!(b.add_term(&Field::Cnae.term(b"2"), &[1]).is_err());
    }

    #[test]
    fn out_of_range_id_is_corrupt() {
        let mut b = DocValuesBuilder::new(&[Field::Cnae], 4);
        b.add_term(&Field::Cnae.term(b"1"), &[0]).unwrap();
        b.add_term(&Field::Cnae.term(b"2"), &[1]).unwrap();
        let mut file = b.finish().unwrap();
        let cols = parse(&file, 0, file.len(), 4).unwrap();
        assert_eq!(cols[0].bits, 2);
        // Doc 3 occupies bits 6..8 of the first packed byte: id 3 has no value.
        file[cols[0].ids.0] |= 0b1100_0000;
        assert_eq!(cols[0].id(&file, 3), Some(3));
        assert!(cols[0].count(&file, 0..4).is_err());
        assert!(cols[0].count(&file, 4..5).is_err(), "doc beyond the column");
    }
}
