//! Inverted index: term dictionary (FST) → postings (sorted document ids).
//!
//! A term is `[field byte][value bytes]`. Postings are stored as
//! `varint len, tag, payload`: tag `0` = delta-varint list (small lists), tag `1` = Roaring
//! portable serialization (large or dense lists). The smaller encoding is chosen per term.

use std::io::{BufWriter, Read, Write};

use opencnpj_core::varint::{self, Reader};
use roaring::RoaringBitmap;

use crate::Error;

/// Indexed fields. The discriminant is the first byte of every term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Field {
    /// Tokens of `razao_social` (propagated to every establishment of the company).
    Razao = 1,
    /// Tokens of `nome_fantasia`.
    Fantasia = 2,
    /// Ranking bucket: token count of `razao_social` (0..=15, 15 = 15 or more).
    RazaoLen = 3,
    Uf = 4,
    Municipio = 5,
    Cnae = 6,
    CnaeSecundaria = 7,
    Situacao = 8,
    Porte = 9,
    Natureza = 10,
    Simples = 11,
    Mei = 12,
    MatrizFilial = 13,
    /// Ranking bucket: token count of `nome_fantasia` (1..=15).
    FantasiaLen = 14,
    /// 11-digit numbers in company names: the owner's CPF for individual entrepreneurs/MEI.
    Cpf = 15,
    /// Partner documents: full CNPJ of legal-entity partners, `*` + the 6 visible digits of
    /// the masked CPF of natural persons and legal representatives (`***123456**`).
    SocioDoc = 16,
    Motivo = 17,
    /// 8-digit CEP.
    Cep = 18,
    /// Area code of the first phone (leading zeros removed).
    Ddd = 19,
    /// `email` / `telefone`: the establishment publishes that contact.
    Contato = 20,
    /// Query-only: the CPF as owner (`cpf_titular`) or as a masked partner (never stored).
    CpfQualquer = 98,
    /// Query-only alias: main or secondary CNAE (never stored).
    CnaeQualquer = 99,
}

impl Field {
    pub const FILTERS: [Field; 18] = [
        Field::CpfQualquer,
        Field::Cpf,
        Field::SocioDoc,
        Field::Motivo,
        Field::Cep,
        Field::Ddd,
        Field::Contato,
        Field::CnaeQualquer,
        Field::Uf,
        Field::Municipio,
        Field::Cnae,
        Field::CnaeSecundaria,
        Field::Situacao,
        Field::Porte,
        Field::Natureza,
        Field::Simples,
        Field::Mei,
        Field::MatrizFilial,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Field::Razao => "razao_social",
            Field::Fantasia => "nome_fantasia",
            Field::RazaoLen => "razao_len",
            Field::FantasiaLen => "fantasia_len",
            Field::Uf => "uf",
            Field::Municipio => "municipio",
            Field::Cnae => "cnae",
            Field::CnaeSecundaria => "cnae_secundaria",
            Field::Situacao => "situacao",
            Field::Porte => "porte",
            Field::Natureza => "natureza",
            Field::Simples => "simples",
            Field::Mei => "mei",
            Field::MatrizFilial => "matriz_filial",
            Field::Cpf => "cpf_titular",
            Field::CpfQualquer => "cpf",
            Field::SocioDoc => "socio_documento",
            Field::Motivo => "motivo",
            Field::Cep => "cep",
            Field::Ddd => "ddd",
            Field::Contato => "contato",
            Field::CnaeQualquer => "cnae_qualquer",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [Field::Razao, Field::Fantasia, Field::RazaoLen, Field::FantasiaLen].into_iter().chain(Self::FILTERS).find(|f| f.name() == s)
    }

    pub fn term(self, value: &[u8]) -> Vec<u8> {
        let mut t = Vec::with_capacity(1 + value.len());
        t.push(self as u8);
        t.extend_from_slice(value);
        t
    }
}

pub const MAX_NAME_LEN_BUCKET: u8 = 15;

const TAG_LIST: u8 = 0;
const TAG_ROARING: u8 = 1;

/// Encodes sorted, unique doc ids, choosing the smaller representation.
pub fn encode_postings(docs: &[u32], out: &mut Vec<u8>) -> Result<(), Error> {
    let mut list = Vec::with_capacity(docs.len() * 2 + 4);
    varint::put(&mut list, docs.len() as u64);
    let mut prev = 0u32;
    for (i, &d) in docs.iter().enumerate() {
        debug_assert!(i == 0 || d > prev);
        varint::put(&mut list, u64::from(if i == 0 { d } else { d - prev }));
        prev = d;
    }
    let body = if docs.len() <= 4 {
        (TAG_LIST, list)
    } else {
        let bm = RoaringBitmap::from_sorted_iter(docs.iter().copied()).map_err(|_| Error::Corrupt("unsorted postings"))?;
        if bm.serialized_size() < list.len() {
            let mut b = Vec::with_capacity(bm.serialized_size());
            bm.serialize_into(&mut b)?;
            (TAG_ROARING, b)
        } else {
            (TAG_LIST, list)
        }
    };
    varint::put(out, body.1.len() as u64 + 1);
    out.push(body.0);
    out.extend_from_slice(&body.1);
    Ok(())
}

/// Decodes postings at `offset` within the postings section.
/// Returns the bitmap and its encoded size.
pub fn decode_postings(section: &[u8], offset: usize, max_doc: u32) -> Result<(RoaringBitmap, usize), Error> {
    let mut r = Reader::new(section.get(offset..).ok_or(Error::Corrupt("postings offset"))?);
    let len = r.varint_usize()?;
    let body = r.bytes(len)?;
    let (&tag, payload) = body.split_first().ok_or(Error::Corrupt("empty postings"))?;
    let bm = match tag {
        TAG_LIST => {
            let mut r = Reader::new(payload);
            let n = r.varint_usize()?;
            if n > payload.len() {
                return Err(Error::Corrupt("postings count"));
            }
            let mut docs = Vec::with_capacity(n);
            let mut cur: u64 = 0;
            for i in 0..n {
                let d = r.varint()?;
                cur = if i == 0 { d } else { cur.checked_add(d).filter(|_| d > 0).ok_or(Error::Corrupt("postings order"))? };
                docs.push(u32::try_from(cur).map_err(|_| Error::Corrupt("doc id"))?);
            }
            RoaringBitmap::from_sorted_iter(docs).map_err(|_| Error::Corrupt("postings order"))?
        }
        TAG_ROARING => RoaringBitmap::deserialize_from(payload)?,
        _ => return Err(Error::Corrupt("postings tag")),
    };
    if bm.max().is_some_and(|m| m >= max_doc) {
        return Err(Error::Corrupt("doc id out of range"));
    }
    Ok((bm, len))
}

/// Collects `(term, doc)` pairs for one bucket with interned terms (8 bytes per pair).
#[derive(Default)]
pub struct TermCollector {
    ids: std::collections::HashMap<Vec<u8>, u32>,
    terms: Vec<Vec<u8>>,
    pairs: Vec<(u32, u32)>,
    key: Vec<u8>,
}

impl TermCollector {
    pub fn add(&mut self, field: Field, value: &[u8], doc: u32) {
        self.key.clear();
        self.key.push(field as u8);
        self.key.extend_from_slice(value);
        let id = match self.ids.get(self.key.as_slice()) {
            Some(&id) => id,
            None => {
                let id = self.terms.len() as u32;
                self.terms.push(self.key.clone());
                self.ids.insert(self.key.clone(), id);
                id
            }
        };
        self.pairs.push((id, doc));
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Writes the sorted run; returns the number of distinct terms.
    pub fn write_run(self, out: impl Write) -> Result<u64, Error> {
        let Self { terms, mut pairs, .. } = self;
        let mut order: Vec<u32> = (0..terms.len() as u32).collect();
        order.sort_unstable_by(|a, b| terms[*a as usize].cmp(&terms[*b as usize]));
        let mut rank = vec![0u32; terms.len()];
        for (r, id) in order.iter().enumerate() {
            rank[*id as usize] = r as u32;
        }
        for p in &mut pairs {
            p.0 = rank[p.0 as usize];
        }
        pairs.sort_unstable();
        pairs.dedup();
        let mut w = zstd::stream::write::Encoder::new(BufWriter::with_capacity(1 << 16, out), 1)?;
        let mut buf = Vec::with_capacity(1 << 16);
        let mut i = 0;
        while i < pairs.len() {
            let mut j = i;
            while j < pairs.len() && pairs[j].0 == pairs[i].0 {
                j += 1;
            }
            varint::put_lp(&mut buf, &terms[order[pairs[i].0 as usize] as usize]);
            varint::put(&mut buf, (j - i) as u64);
            let mut prev = 0;
            for (_, d) in &pairs[i..j] {
                varint::put(&mut buf, u64::from(d - prev));
                prev = *d;
            }
            if buf.len() >= 1 << 15 {
                w.write_all(&buf)?;
                buf.clear();
            }
            i = j;
        }
        w.write_all(&buf)?;
        w.finish()?.flush()?;
        Ok(terms.len() as u64)
    }
}

/// Writes one bucket's `(term, local doc)` pairs as a sorted run: `lp(term) varint(n) deltas`.
pub fn write_run(mut pairs: Vec<(Vec<u8>, u32)>, out: impl Write) -> Result<u64, Error> {
    pairs.sort_unstable();
    pairs.dedup();
    let mut w = zstd::stream::write::Encoder::new(BufWriter::with_capacity(1 << 16, out), 1)?;
    let mut buf = Vec::with_capacity(1 << 16);
    let mut terms = 0u64;
    let mut i = 0;
    while i < pairs.len() {
        let mut j = i;
        while j < pairs.len() && pairs[j].0 == pairs[i].0 {
            j += 1;
        }
        varint::put_lp(&mut buf, &pairs[i].0);
        varint::put(&mut buf, (j - i) as u64);
        let mut prev = 0;
        for (_, d) in &pairs[i..j] {
            varint::put(&mut buf, u64::from(d - prev));
            prev = *d;
        }
        terms += 1;
        if buf.len() >= 1 << 15 {
            w.write_all(&buf)?;
            buf.clear();
        }
        i = j;
    }
    w.write_all(&buf)?;
    w.finish()?.flush()?;
    Ok(terms)
}

/// Streaming reader for a run file.
pub struct RunReader<R: Read> {
    inner: std::io::BufReader<zstd::stream::read::Decoder<'static, std::io::BufReader<R>>>,
    pub base: u32,
}

impl<R: Read> RunReader<R> {
    pub fn new(r: R, base: u32) -> Result<Self, Error> {
        Ok(Self { inner: std::io::BufReader::with_capacity(1 << 16, zstd::stream::read::Decoder::new(r)?), base })
    }

    fn varint(&mut self) -> Result<Option<u64>, Error> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let mut b = [0u8];
            if self.inner.read(&mut b)? == 0 {
                return if shift == 0 { Ok(None) } else { Err(Error::Corrupt("truncated run")) };
            }
            v |= u64::from(b[0] & 0x7f) << shift;
            if b[0] < 0x80 {
                return Ok(Some(v));
            }
        }
        Err(Error::Corrupt("bad varint in run"))
    }

    /// Next `(term, docs)` with docs offset by `base`.
    pub fn next_term(&mut self, term: &mut Vec<u8>, docs: &mut Vec<u32>) -> Result<bool, Error> {
        let Some(len) = self.varint()? else { return Ok(false) };
        term.resize(len as usize, 0);
        self.inner.read_exact(term)?;
        let n = self.varint()?.ok_or(Error::Corrupt("truncated run"))?;
        docs.clear();
        let mut cur = 0u64;
        for _ in 0..n {
            cur += self.varint()?.ok_or(Error::Corrupt("truncated run"))?;
            docs.push(self.base + cur as u32);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postings_roundtrip_both_encodings() {
        for docs in [vec![7u32], vec![1, 5, 9, 1000], (0..50_000).step_by(3).collect::<Vec<_>>(), (10..20_000).collect()] {
            let mut buf = vec![0xAA];
            encode_postings(&docs, &mut buf).unwrap();
            let (bm, _) = decode_postings(&buf, 1, 1 << 30).unwrap();
            assert_eq!(bm.iter().collect::<Vec<_>>(), docs);
            assert!(decode_postings(&buf, 1, *docs.last().unwrap()).is_err());
        }
    }

    #[test]
    fn collector_matches_plain_run() {
        let mut c = TermCollector::default();
        c.add(Field::Razao, b"B", 3);
        c.add(Field::Razao, b"A", 9);
        c.add(Field::Razao, b"A", 2);
        c.add(Field::Razao, b"A", 2);
        let mut a = Vec::new();
        c.write_run(&mut a).unwrap();
        let mut b = Vec::new();
        write_run(vec![(b"\x01B".to_vec(), 3), (b"\x01A".to_vec(), 9), (b"\x01A".to_vec(), 2)], &mut b).unwrap();
        let read = |f: &[u8]| {
            let mut r = RunReader::new(f, 0).unwrap();
            let (mut t, mut d, mut v) = (Vec::new(), Vec::new(), Vec::new());
            while r.next_term(&mut t, &mut d).unwrap() {
                v.push((t.clone(), d.clone()));
            }
            v
        };
        assert_eq!(read(&a), read(&b));
    }

    #[test]
    fn run_roundtrip() {
        let pairs = vec![(b"\x01B".to_vec(), 3), (b"\x01A".to_vec(), 9), (b"\x01A".to_vec(), 2), (b"\x01A".to_vec(), 2)];
        let mut file = Vec::new();
        assert_eq!(write_run(pairs, &mut file).unwrap(), 2);
        let mut r = RunReader::new(&file[..], 100).unwrap();
        let (mut t, mut d) = (Vec::new(), Vec::new());
        assert!(r.next_term(&mut t, &mut d).unwrap());
        assert_eq!((t.as_slice(), d.as_slice()), (&b"\x01A"[..], &[102u32, 109][..]));
        assert!(r.next_term(&mut t, &mut d).unwrap());
        assert_eq!(d, [103]);
        assert!(!r.next_term(&mut t, &mut d).unwrap());
    }
}
