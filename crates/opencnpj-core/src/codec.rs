//! Lossless per-field codecs.
//!
//! Typed fields are written as a varint tag: `0` = empty string, `1` = escaped (the
//! verbatim string follows, length-prefixed), `n >= 2` = typed value `n - 2` (plus a
//! payload for lists). A typed encoding is only chosen when formatting it back yields
//! the exact source string, so decoding always reproduces the original value.

use std::collections::HashMap;

use crate::schema::Kind;
use crate::varint::{self, Malformed, Reader};

const TAG_EMPTY: u64 = 0;
const TAG_RAW: u64 = 1;
const TYPED: u64 = 2;

/// Global dictionary for a `Kind::Dict` column, ordered by descending frequency.
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    pub values: Vec<String>,
    index: HashMap<String, u32>,
}

impl Dictionary {
    pub fn new(values: Vec<String>) -> Self {
        let index = values.iter().enumerate().map(|(i, v)| (v.clone(), i as u32)).collect();
        Self { values, index }
    }

    pub fn id(&self, value: &str) -> Option<u32> {
        self.index.get(value).copied()
    }
}

#[derive(Debug, Clone)]
pub struct ColumnCodec {
    pub kind: crate::schema::Kind,
    pub dict: Option<Dictionary>,
}

impl ColumnCodec {
    pub fn new(kind: Kind, dict: Option<Dictionary>) -> Self {
        Self { kind, dict }
    }

    pub fn encode(&self, s: &str, out: &mut Vec<u8>) {
        if self.kind == Kind::Text {
            varint::put_lp(out, s.as_bytes());
            return;
        }
        if s.is_empty() {
            varint::put(out, TAG_EMPTY);
            return;
        }
        let typed = match self.kind {
            Kind::Text => unreachable!(),
            Kind::Dict => self.dict.as_ref().and_then(|d| d.id(s)).map(u64::from),
            Kind::Date => encode_date(s),
            Kind::Money => encode_money(s),
            Kind::Digits(n) => encode_digits(s, n),
            Kind::CnaeList => {
                if let Some(items) = parse_cnae_list(s) {
                    varint::put(out, TYPED + items.len() as u64);
                    for item in items {
                        varint::put(out, u64::from(item));
                    }
                    return;
                }
                None
            }
        };
        match typed {
            Some(v) => varint::put(out, TYPED + v),
            None => {
                varint::put(out, TAG_RAW);
                varint::put_lp(out, s.as_bytes());
            }
        }
    }

    /// Appends the decoded value to `out` (which the caller clears).
    pub fn decode(&self, r: &mut Reader<'_>, out: &mut String) -> Result<(), Malformed> {
        if self.kind == Kind::Text {
            out.push_str(std::str::from_utf8(r.lp_bytes()?).map_err(|_| Malformed)?);
            return Ok(());
        }
        let tag = r.varint()?;
        match tag {
            TAG_EMPTY => Ok(()),
            TAG_RAW => {
                out.push_str(std::str::from_utf8(r.lp_bytes()?).map_err(|_| Malformed)?);
                Ok(())
            }
            t => {
                let v = t - TYPED;
                match self.kind {
                    Kind::Text => unreachable!(),
                    Kind::Dict => {
                        let d = self.dict.as_ref().ok_or(Malformed)?;
                        out.push_str(d.values.get(usize::try_from(v).map_err(|_| Malformed)?).ok_or(Malformed)?);
                    }
                    Kind::Date => decode_date(v, out)?,
                    Kind::Money => {
                        use std::fmt::Write;
                        write!(out, "{},{:02}", v / 100, v % 100).expect("string write");
                    }
                    Kind::Digits(n) => {
                        use std::fmt::Write;
                        if v >= 10u64.pow(u32::from(n)) {
                            return Err(Malformed);
                        }
                        write!(out, "{:0width$}", v, width = usize::from(n)).expect("string write");
                    }
                    Kind::CnaeList => {
                        if v == 0 || v > 4096 {
                            return Err(Malformed);
                        }
                        for i in 0..v {
                            if i > 0 {
                                out.push(',');
                            }
                            let item = r.varint()?;
                            if item >= 10_000_000 {
                                return Err(Malformed);
                            }
                            use std::fmt::Write;
                            write!(out, "{item:07}").expect("string write");
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

fn encode_digits(s: &str, n: u8) -> Option<u64> {
    (s.len() == usize::from(n) && n <= 18 && all_digits(s)).then(|| s.parse().expect("digits"))
}

/// `1000,00` → 100000 cents. Only canonical forms (no leading zeros, two fraction digits).
fn encode_money(s: &str) -> Option<u64> {
    let (int, frac) = s.split_once(',')?;
    if int.is_empty() || int.len() > 16 || frac.len() != 2 || !all_digits(int) || !all_digits(frac) {
        return None;
    }
    if int.len() > 1 && int.starts_with('0') {
        return None;
    }
    let int: u64 = int.parse().ok()?;
    int.checked_mul(100)?.checked_add(frac.parse::<u64>().ok()?)
}

/// Days since 1000-01-01 plus one; `00000000` → 0.
fn encode_date(s: &str) -> Option<u64> {
    if s.len() != 8 || !all_digits(s) {
        return None;
    }
    if s == "00000000" {
        return Some(0);
    }
    let y: i64 = s[..4].parse().ok()?;
    let m: u32 = s[4..6].parse().ok()?;
    let d: u32 = s[6..].parse().ok()?;
    if y < 1000 || !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some((days_from_civil(y, m, d) - EPOCH_1000) as u64 + 1)
}

fn decode_date(v: u64, out: &mut String) -> Result<(), Malformed> {
    use std::fmt::Write;
    if v == 0 {
        out.push_str("00000000");
        return Ok(());
    }
    if v > 3_000_000 {
        return Err(Malformed);
    }
    let (y, m, d) = civil_from_days(v as i64 - 1 + EPOCH_1000);
    if y > 9999 {
        return Err(Malformed);
    }
    write!(out, "{y:04}{m:02}{d:02}").expect("string write");
    Ok(())
}

const EPOCH_1000: i64 = -354_285; // days_from_civil(1000, 1, 1)

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Howard Hinnant's algorithm: days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parses `1234567,7654321` into codes; `None` unless every item is exactly 7 digits.
pub fn parse_cnae_list(s: &str) -> Option<Vec<u32>> {
    let mut items = Vec::new();
    for item in s.split(',') {
        if item.len() != 7 || !all_digits(item) {
            return None;
        }
        items.push(item.parse().expect("digits"));
    }
    Some(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn roundtrip(codec: &ColumnCodec, s: &str) -> (String, usize) {
        let mut buf = Vec::new();
        codec.encode(s, &mut buf);
        let mut r = Reader::new(&buf);
        let mut out = String::new();
        codec.decode(&mut r, &mut out).unwrap();
        assert!(r.is_empty(), "trailing bytes for {s:?}");
        (out, buf.len())
    }

    #[test]
    fn epoch_constant() {
        assert_eq!(days_from_civil(1000, 1, 1), EPOCH_1000);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn dates() {
        let c = ColumnCodec::new(Kind::Date, None);
        for s in ["20070402", "00000000", "", "20240229", "20230229", "2007040", "19000101", "00010101", "abc"] {
            assert_eq!(roundtrip(&c, s).0, s);
        }
        assert!(roundtrip(&c, "20070402").1 <= 3);
    }

    #[test]
    fn money() {
        let c = ColumnCodec::new(Kind::Money, None);
        for s in ["0,00", "1000,00", "120000000000,00", "01,00", "1,5", "1.000,00", "-5,00", "", "99999999999999999,99"] {
            assert_eq!(roundtrip(&c, s).0, s);
        }
    }

    #[test]
    fn digits_and_lists() {
        let cep = ColumnCodec::new(Kind::Digits(8), None);
        for s in ["01310100", "1310100", "0131010A", ""] {
            assert_eq!(roundtrip(&cep, s).0, s);
        }
        let l = ColumnCodec::new(Kind::CnaeList, None);
        for s in ["7020400,7490105", "0111301", "7020400,", "702040", ""] {
            assert_eq!(roundtrip(&l, s).0, s);
        }
    }

    #[test]
    fn dict() {
        let c = ColumnCodec::new(Kind::Dict, Some(Dictionary::new(vec!["SP".into(), "RJ".into()])));
        for s in ["SP", "RJ", "MG", ""] {
            assert_eq!(roundtrip(&c, s).0, s);
        }
    }

    proptest! {
        #[test]
        fn any_string_roundtrips_for_every_kind(s in ".{0,40}") {
            let kinds = [Kind::Text, Kind::Dict, Kind::Date, Kind::Money, Kind::Digits(2), Kind::Digits(8), Kind::CnaeList];
            for kind in kinds {
                let c = ColumnCodec::new(kind, Some(Dictionary::new(vec![s.clone()])));
                prop_assert_eq!(roundtrip(&c, &s).0, s.clone());
            }
        }

        #[test]
        fn numeric_strings_roundtrip(s in "[0-9,]{0,20}") {
            for kind in [Kind::Date, Kind::Money, Kind::Digits(8), Kind::CnaeList] {
                let c = ColumnCodec::new(kind, None);
                prop_assert_eq!(roundtrip(&c, &s).0, s.clone());
            }
        }

        #[test]
        fn decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            for kind in [Kind::Text, Kind::Dict, Kind::Date, Kind::Money, Kind::Digits(8), Kind::CnaeList] {
                let c = ColumnCodec::new(kind, Some(Dictionary::new(vec!["A".into()])));
                let mut out = String::new();
                let _ = c.decode(&mut Reader::new(&bytes), &mut out);
            }
        }
    }
}
