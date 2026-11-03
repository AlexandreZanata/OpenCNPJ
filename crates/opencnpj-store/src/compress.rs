//! Block compression. Every block is an independent frame so a lookup never decompresses
//! more than one block.

use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "codec", rename_all = "snake_case")]
pub enum BlockCodec {
    None,
    Lz4,
    Zstd { level: i32 },
}

impl BlockCodec {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "lz4" => Self::Lz4,
            _ => Self::Zstd { level: s.strip_prefix("zstd")?.trim_start_matches(['-', ':']).parse().ok()? },
        })
    }

    pub fn label(&self) -> String {
        match self {
            Self::None => "none".into(),
            Self::Lz4 => "lz4".into(),
            Self::Zstd { level } => format!("zstd{level}"),
        }
    }
}

/// Reusable compressor (one per thread).
pub struct BlockCompressor {
    codec: BlockCodec,
    zstd: Option<zstd::bulk::Compressor<'static>>,
}

impl BlockCompressor {
    pub fn new(codec: BlockCodec, dict: Option<&[u8]>) -> Result<Self, Error> {
        let zstd = match codec {
            BlockCodec::Zstd { level } => Some(match dict {
                Some(d) => zstd::bulk::Compressor::with_dictionary(level, d)?,
                None => zstd::bulk::Compressor::new(level)?,
            }),
            _ => None,
        };
        Ok(Self { codec, zstd })
    }

    pub fn compress(&mut self, raw: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(match self.codec {
            BlockCodec::None => raw.to_vec(),
            BlockCodec::Lz4 => lz4_flex::block::compress(raw),
            BlockCodec::Zstd { .. } => self.zstd.as_mut().expect("zstd compressor").compress(raw)?,
        })
    }
}

/// Shared, immutable decompression state.
pub struct BlockDecompressor {
    codec: BlockCodec,
    dict: Option<zstd::dict::DecoderDictionary<'static>>,
}

impl BlockDecompressor {
    pub fn new(codec: BlockCodec, dict: Option<&[u8]>) -> Self {
        Self { codec, dict: dict.map(zstd::dict::DecoderDictionary::copy) }
    }

    /// Decompresses into exactly `raw_len` bytes or fails.
    pub fn decompress(&self, data: &[u8], raw_len: usize) -> Result<Vec<u8>, Error> {
        if raw_len > crate::format::MAX_BLOCK_RAW {
            return Err(Error::Corrupt("block raw length exceeds limit"));
        }
        let out = match self.codec {
            BlockCodec::None => data.to_vec(),
            BlockCodec::Lz4 => lz4_flex::block::decompress(data, raw_len).map_err(|_| Error::Corrupt("lz4 block"))?,
            BlockCodec::Zstd { .. } => {
                let mut d = match &self.dict {
                    Some(dd) => zstd::bulk::Decompressor::with_prepared_dictionary(dd)?,
                    None => zstd::bulk::Decompressor::new()?,
                };
                d.decompress(data, raw_len).map_err(|_| Error::Corrupt("zstd block"))?
            }
        };
        if out.len() != raw_len {
            return Err(Error::Corrupt("block length mismatch after decompression"));
        }
        Ok(out)
    }
}

/// Trains a zstd dictionary from sample blocks.
pub fn train_dictionary(samples: &[Vec<u8>], max_size: usize) -> Result<Vec<u8>, Error> {
    Ok(zstd::dict::from_samples(samples, max_size)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codecs_roundtrip() {
        let raw: Vec<u8> = (0..10_000u32).flat_map(|i| (i % 97).to_le_bytes()).collect();
        for codec in [BlockCodec::None, BlockCodec::Lz4, BlockCodec::Zstd { level: 3 }] {
            let c = BlockCompressor::new(codec, None).unwrap().compress(&raw).unwrap();
            let d = BlockDecompressor::new(codec, None);
            assert_eq!(d.decompress(&c, raw.len()).unwrap(), raw);
            assert!(d.decompress(&c, raw.len() + 1).is_err());
        }
        assert_eq!(BlockCodec::parse("zstd19"), Some(BlockCodec::Zstd { level: 19 }));
        assert_eq!(BlockCodec::parse("zstd-3"), Some(BlockCodec::Zstd { level: 3 }));
        assert_eq!(BlockCodec::parse("lz4"), Some(BlockCodec::Lz4));
    }
}
