//! `.ocnpj` snapshot format: columnar compressed blocks, block directory, inverted index.

pub mod block;
pub mod compress;
pub mod docvalues;
pub mod format;
pub mod index;
pub mod manifest;
pub mod postings;
pub mod reader;
pub mod search;
pub mod writer;

pub use reader::{DocRecord, Snapshot};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt snapshot: {0}")]
    Corrupt(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

impl From<opencnpj_core::varint::Malformed> for Error {
    fn from(_: opencnpj_core::varint::Malformed) -> Self {
        Error::Corrupt("truncated or malformed encoded data")
    }
}
