//! CNPJ identifiers, numeric and alphanumeric (IN RFB nº 2.229/2024).
//!
//! A CNPJ has 14 characters: 8 for the root (`basico`), 4 for the branch
//! (`ordem`) and 2 check digits. Since July 2026 the first 12 positions may be
//! `0-9` or `A-Z`; the check digits stay numeric. The check digit algorithm is
//! the classic modulo 11 with each character valued as `ASCII - 48`, which keeps
//! numeric CNPJs computing exactly as before.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CnpjError {
    #[error("cnpj must have 14 characters after removing punctuation")]
    Length,
    #[error("cnpj contains an invalid character")]
    Charset,
    #[error("cnpj check digits do not match")]
    CheckDigits,
}

/// A validated 14-character CNPJ stored as uppercase ASCII.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cnpj([u8; 14]);

impl Cnpj {
    /// Parses user input, ignoring `.`, `/`, `-` and whitespace, and accepting lowercase letters.
    pub fn parse(input: &str) -> Result<Self, CnpjError> {
        let mut out = [0u8; 14];
        let mut n = 0;
        for b in input.bytes() {
            if matches!(b, b'.' | b'/' | b'-' | b' ' | b'\t') {
                continue;
            }
            if n == 14 {
                return Err(CnpjError::Length);
            }
            out[n] = b.to_ascii_uppercase();
            n += 1;
        }
        if n != 14 {
            return Err(CnpjError::Length);
        }
        Self::from_raw(out)
    }

    /// Validates a raw 14-byte identifier (no punctuation).
    pub fn from_raw(raw: [u8; 14]) -> Result<Self, CnpjError> {
        if !raw[..12].iter().all(|b| is_base_char(*b)) || !raw[12..].iter().all(u8::is_ascii_digit) {
            return Err(CnpjError::Charset);
        }
        let base: &[u8; 12] = raw[..12].try_into().expect("12 bytes");
        if check_digits(base) != [raw[12], raw[13]] {
            return Err(CnpjError::CheckDigits);
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("validated ascii")
    }

    pub fn basico(&self) -> &str {
        &self.as_str()[..8]
    }

    pub fn ordem(&self) -> &str {
        &self.as_str()[8..12]
    }

    pub fn dv(&self) -> &str {
        &self.as_str()[12..]
    }

    pub fn is_alphanumeric(&self) -> bool {
        self.0[..12].iter().any(u8::is_ascii_uppercase)
    }

    /// `XX.XXX.XXX/XXXX-XX`
    pub fn formatted(&self) -> String {
        let s = self.as_str();
        format!("{}.{}.{}/{}-{}", &s[..2], &s[2..5], &s[5..8], &s[8..12], &s[12..])
    }
}

impl fmt::Display for Cnpj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Cnpj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cnpj({})", self.as_str())
    }
}

fn is_base_char(b: u8) -> bool {
    b.is_ascii_digit() || b.is_ascii_uppercase()
}

/// Computes the two check digits for the first 12 characters.
pub fn check_digits(base: &[u8; 12]) -> [u8; 2] {
    const W1: [u32; 12] = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    const W2: [u32; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let value = |b: u8| u32::from(b) - 48;
    let digit = |sum: u32| {
        let r = sum % 11;
        if r < 2 { 0 } else { 11 - r }
    };
    let d1 = digit(base.iter().zip(W1).map(|(b, w)| value(*b) * w).sum());
    let d2 = digit(base.iter().chain(std::iter::once(&(d1 as u8 + b'0'))).zip(W2).map(|(b, w)| value(*b) * w).sum());
    [d1 as u8 + b'0', d2 as u8 + b'0']
}

/// Validates an 8-character CNPJ root (`basico`), numeric or alphanumeric.
pub fn parse_basico(input: &str) -> Option<[u8; 8]> {
    let mut out = [0u8; 8];
    let mut n = 0;
    for b in input.bytes() {
        if matches!(b, b'.' | b' ') {
            continue;
        }
        let b = b.to_ascii_uppercase();
        if n == 8 || !is_base_char(b) {
            return None;
        }
        out[n] = b;
        n += 1;
    }
    (n == 8).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_known_values() {
        assert!(Cnpj::parse("00.000.000/0001-91").is_ok()); // Banco do Brasil
        assert!(Cnpj::parse("33000167000101").is_ok()); // Petrobras
        assert_eq!(Cnpj::parse("33000167000102"), Err(CnpjError::CheckDigits));
        assert_eq!(Cnpj::parse("3300016700010"), Err(CnpjError::Length));
    }

    #[test]
    fn alphanumeric_reference_example() {
        // Example published by RFB for the alphanumeric CNPJ.
        let c = Cnpj::parse("12.ABC.345/01DE-35").unwrap();
        assert!(c.is_alphanumeric());
        assert_eq!(c.basico(), "12ABC345");
        assert_eq!(c.ordem(), "01DE");
        assert_eq!(c.formatted(), "12.ABC.345/01DE-35");
        assert!(Cnpj::parse("12abc34501de35").is_ok());
        assert_eq!(Cnpj::parse("12ABC34501DE36"), Err(CnpjError::CheckDigits));
        assert_eq!(Cnpj::parse("12ABC34501DE3A"), Err(CnpjError::Charset));
    }

    #[test]
    fn basico_parse() {
        assert_eq!(parse_basico("33.000.167"), Some(*b"33000167"));
        assert_eq!(parse_basico("12abc345"), Some(*b"12ABC345"));
        assert_eq!(parse_basico("1234567"), None);
        assert_eq!(parse_basico("1234567*"), None);
    }
}
