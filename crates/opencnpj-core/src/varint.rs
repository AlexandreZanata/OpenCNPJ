//! LEB128 unsigned varints with strict bounds checks for untrusted input.

#[inline]
pub fn put(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

#[inline]
pub fn len(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

/// Byte cursor over untrusted data. Every read is bounds-checked.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("truncated or malformed binary data")]
pub struct Malformed;

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    #[inline]
    pub fn varint(&mut self) -> Result<u64, Malformed> {
        let mut v: u64 = 0;
        for shift in (0..64).step_by(7) {
            let b = *self.buf.get(self.pos).ok_or(Malformed)?;
            self.pos += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b < 0x80 {
                return Ok(v);
            }
        }
        Err(Malformed)
    }

    #[inline]
    pub fn varint_usize(&mut self) -> Result<usize, Malformed> {
        usize::try_from(self.varint()?).map_err(|_| Malformed)
    }

    #[inline]
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let end = self.pos.checked_add(n).ok_or(Malformed)?;
        let s = self.buf.get(self.pos..end).ok_or(Malformed)?;
        self.pos = end;
        Ok(s)
    }

    /// Length-prefixed byte string.
    #[inline]
    pub fn lp_bytes(&mut self) -> Result<&'a [u8], Malformed> {
        let n = self.varint_usize()?;
        self.bytes(n)
    }

    pub fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u32_le(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().expect("4 bytes")))
    }

    pub fn u64_le(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().expect("8 bytes")))
    }
}

#[inline]
pub fn put_lp(out: &mut Vec<u8>, bytes: &[u8]) {
    put(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_edges() {
        for v in [0, 1, 127, 128, 16_383, 16_384, u32::MAX as u64, u64::MAX] {
            let mut b = Vec::new();
            put(&mut b, v);
            assert_eq!(b.len(), len(v));
            assert_eq!(Reader::new(&b).varint().unwrap(), v);
        }
    }

    #[test]
    fn rejects_truncated_and_overlong() {
        assert!(Reader::new(&[0x80]).varint().is_err());
        assert!(Reader::new(&[0xff; 11]).varint().is_err());
        assert!(Reader::new(&[5, 1, 2]).lp_bytes().is_err());
    }
}
