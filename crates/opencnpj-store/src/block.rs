//! Columnar blocks of whole companies.
//!
//! A block holds consecutive companies (sorted by `cnpj_basico`) with all their rows.
//! Raw layout: `varint n_companies`, `varint n_streams`, `n_streams × varint len`, then the
//! streams. Stream 0 holds keys, streams 1..=4 per-company row counts (one per table),
//! then one stream per row column of each table, in schema order.

use opencnpj_core::codec::ColumnCodec;
use opencnpj_core::varint::{self, Malformed, Reader};
use opencnpj_core::{Company, Table};

use crate::Error;

const KEY_RAW: u64 = 0;

/// Codecs for every row column of every table.
#[derive(Debug, Clone)]
pub struct Codecs {
    pub tables: [Vec<ColumnCodec>; 4],
}

impl Codecs {
    pub fn n_streams(&self) -> usize {
        5 + self.tables.iter().map(Vec::len).sum::<usize>()
    }

    pub fn table(&self, t: Table) -> &[ColumnCodec] {
        &self.tables[t.index()]
    }
}

/// Accumulates companies into column streams.
pub struct BlockBuilder<'a> {
    codecs: &'a Codecs,
    streams: Vec<Vec<u8>>,
    companies: u32,
    docs: u32,
    prev_numeric: u64,
    first_key: Option<[u8; 8]>,
}

impl<'a> BlockBuilder<'a> {
    pub fn new(codecs: &'a Codecs) -> Self {
        Self { codecs, streams: vec![Vec::new(); codecs.n_streams()], companies: 0, docs: 0, prev_numeric: 0, first_key: None }
    }

    pub fn is_empty(&self) -> bool {
        self.companies == 0
    }

    pub fn companies(&self) -> u32 {
        self.companies
    }

    pub fn docs(&self) -> u32 {
        self.docs
    }

    pub fn first_key(&self) -> [u8; 8] {
        self.first_key.unwrap_or_default()
    }

    pub fn raw_len(&self) -> usize {
        self.streams.iter().map(Vec::len).sum()
    }

    /// Keys must be pushed in strictly ascending byte order and be exactly 8 bytes.
    pub fn push(&mut self, c: &Company) {
        let key = c.basico.as_bytes();
        debug_assert_eq!(key.len(), 8);
        if self.first_key.is_none() {
            self.first_key = Some(key.try_into().expect("8-byte key"));
        }
        // Numeric keys are delta-coded against the previous numeric key in the block.
        let ks = &mut self.streams[0];
        let numeric = if key.iter().all(u8::is_ascii_digit) { c.basico.parse::<u64>().ok() } else { None };
        match numeric {
            Some(n) if n > self.prev_numeric || (n == 0 && self.companies == 0) => {
                varint::put(ks, n - self.prev_numeric + 1);
                self.prev_numeric = n;
            }
            _ => {
                varint::put(ks, KEY_RAW);
                varint::put_lp(ks, key);
            }
        }
        let mut s = 5;
        for t in Table::ALL {
            let rows = c.rows(t);
            varint::put(&mut self.streams[1 + t.index()], rows.len() as u64);
            let codecs = self.codecs.table(t);
            for row in rows {
                debug_assert_eq!(row.len(), codecs.len());
                for (i, codec) in codecs.iter().enumerate() {
                    codec.encode(&row[i], &mut self.streams[s + i]);
                }
            }
            s += codecs.len();
        }
        self.companies += 1;
        self.docs += c.doc_count();
    }

    /// Serializes and resets the builder.
    pub fn finish(&mut self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw_len() + 16 + self.streams.len() * 3);
        varint::put(&mut out, u64::from(self.companies));
        varint::put(&mut out, self.streams.len() as u64);
        for s in &self.streams {
            varint::put(&mut out, s.len() as u64);
        }
        for s in &mut self.streams {
            out.extend_from_slice(s);
            s.clear();
        }
        self.companies = 0;
        self.docs = 0;
        self.prev_numeric = 0;
        self.first_key = None;
        out
    }
}

/// Parsed stream table of a raw block.
pub struct BlockView<'a> {
    pub companies: usize,
    streams: Vec<&'a [u8]>,
}

impl<'a> BlockView<'a> {
    pub fn parse(raw: &'a [u8], codecs: &Codecs) -> Result<Self, Error> {
        let mut r = Reader::new(raw);
        let companies = r.varint_usize()?;
        let n = r.varint_usize()?;
        if n != codecs.n_streams() {
            return Err(Error::Corrupt("block stream count does not match schema"));
        }
        let lens = (0..n).map(|_| r.varint_usize()).collect::<Result<Vec<_>, _>>()?;
        let mut streams = Vec::with_capacity(n);
        for len in lens {
            streams.push(r.bytes(len)?);
        }
        if !r.is_empty() {
            return Err(Error::Corrupt("trailing bytes in block"));
        }
        if companies > raw.len() {
            return Err(Error::Corrupt("company count exceeds block size"));
        }
        Ok(Self { companies, streams })
    }

    /// Decodes every key in order.
    pub fn keys(&self) -> Result<Vec<String>, Error> {
        let mut r = Reader::new(self.streams[0]);
        let mut prev = 0u64;
        let mut out = Vec::with_capacity(self.companies);
        for _ in 0..self.companies {
            out.push(read_key(&mut r, &mut prev)?);
        }
        Ok(out)
    }

    /// Per-company row counts for each table.
    pub fn counts(&self) -> Result<Vec<[u32; 4]>, Error> {
        let mut readers: Vec<Reader<'_>> = (1..=4).map(|i| Reader::new(self.streams[i])).collect();
        let mut out = Vec::with_capacity(self.companies);
        for _ in 0..self.companies {
            let mut c = [0u32; 4];
            for (t, r) in readers.iter_mut().enumerate() {
                c[t] = u32::try_from(r.varint()?).map_err(|_| Malformed)?;
            }
            out.push(c);
        }
        Ok(out)
    }

    /// Decodes the company at `index` (skipping rows of the companies before it).
    pub fn company(&self, codecs: &Codecs, index: usize, key: String, counts: &[[u32; 4]]) -> Result<Company, Error> {
        let mut c = Company { basico: key, ..Default::default() };
        let mut s = 5;
        let mut buf = String::new();
        for t in Table::ALL {
            let skip: u64 = counts[..index].iter().map(|n| u64::from(n[t.index()])).sum();
            let take = counts[index][t.index()] as usize;
            let cols = codecs.table(t);
            let rows = c.rows_mut(t);
            rows.resize_with(take, || Vec::with_capacity(cols.len()));
            for (i, codec) in cols.iter().enumerate() {
                let mut r = Reader::new(self.streams[s + i]);
                for _ in 0..skip {
                    codec.skip(&mut r)?;
                }
                for row in rows.iter_mut() {
                    buf.clear();
                    codec.decode(&mut r, &mut buf)?;
                    row.push(buf.clone());
                }
            }
            s += cols.len();
        }
        Ok(c)
    }

    /// Decodes all companies.
    pub fn all(&self, codecs: &Codecs) -> Result<Vec<Company>, Error> {
        let keys = self.keys()?;
        let counts = self.counts()?;
        let mut out: Vec<Company> = keys.into_iter().map(|basico| Company { basico, ..Default::default() }).collect();
        let mut s = 5;
        let mut buf = String::new();
        for t in Table::ALL {
            let cols = codecs.table(t);
            let mut readers: Vec<Reader<'_>> = (0..cols.len()).map(|i| Reader::new(self.streams[s + i])).collect();
            for (ci, c) in out.iter_mut().enumerate() {
                for _ in 0..counts[ci][t.index()] {
                    let mut row = Vec::with_capacity(cols.len());
                    for (codec, r) in cols.iter().zip(readers.iter_mut()) {
                        buf.clear();
                        codec.decode(r, &mut buf)?;
                        row.push(buf.clone());
                    }
                    c.rows_mut(t).push(row);
                }
            }
            if readers.iter().any(|r| !r.is_empty()) {
                return Err(Error::Corrupt("unconsumed column data in block"));
            }
            s += cols.len();
        }
        Ok(out)
    }
}

fn read_key(r: &mut Reader<'_>, prev: &mut u64) -> Result<String, Error> {
    let v = r.varint()?;
    if v == KEY_RAW {
        let b = r.lp_bytes()?;
        return String::from_utf8(b.to_vec()).map_err(|_| Error::Corrupt("key is not utf-8"));
    }
    let n = prev.checked_add(v - 1).ok_or(Malformed)?;
    if n > 99_999_999 {
        return Err(Error::Corrupt("numeric key out of range"));
    }
    *prev = n;
    Ok(format!("{n:08}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencnpj_core::codec::Dictionary;
    use opencnpj_core::schema::Kind;

    pub(crate) fn test_codecs() -> Codecs {
        let mk = |t: Table| {
            t.row_columns()
                .iter()
                .map(|c| ColumnCodec::new(c.kind, (c.kind == Kind::Dict).then(|| Dictionary::new(vec!["SP".into(), "02".into()]))))
                .collect()
        };
        Codecs { tables: [mk(Table::Empresas), mk(Table::Estabelecimentos), mk(Table::Socios), mk(Table::Simples)] }
    }

    fn company(key: &str, n_est: usize) -> Company {
        let row = |t: Table, seed: usize| -> Vec<String> {
            t.row_columns().iter().enumerate().map(|(i, _)| if i % 3 == 0 { String::new() } else { format!("{key}-{seed}-{i}") }).collect()
        };
        Company {
            basico: key.into(),
            empresas: vec![row(Table::Empresas, 0)],
            estabelecimentos: (0..n_est).map(|i| row(Table::Estabelecimentos, i)).collect(),
            socios: vec![row(Table::Socios, 0), row(Table::Socios, 1)],
            simples: vec![],
        }
    }

    #[test]
    fn roundtrip_and_partial_decode() {
        let codecs = test_codecs();
        let mut b = BlockBuilder::new(&codecs);
        let cs = vec![company("00000000", 2), company("00000007", 0), company("12ABC345", 1), company("13000000", 3)];
        for c in &cs {
            b.push(c);
        }
        assert_eq!(b.docs(), 2 + 1 + 1 + 3);
        let raw = b.finish();
        let v = BlockView::parse(&raw, &codecs).unwrap();
        assert_eq!(v.all(&codecs).unwrap(), cs);
        let keys = v.keys().unwrap();
        let counts = v.counts().unwrap();
        for i in 0..cs.len() {
            assert_eq!(v.company(&codecs, i, keys[i].clone(), &counts).unwrap(), cs[i]);
        }
        // Truncation is detected, never panics.
        for cut in [1, raw.len() / 2, raw.len() - 1] {
            let r = BlockView::parse(&raw[..cut], &codecs).and_then(|v| v.all(&codecs));
            assert!(r.is_err());
        }
    }
}
