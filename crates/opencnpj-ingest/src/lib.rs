//! Ingestion: download, ZIP streaming, RFB CSV parsing and snapshot building.

pub mod build;
pub mod parse;
pub mod partition;
pub mod source;
#[doc(hidden)]
pub mod testkit;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("source: {0}")]
    Source(String),
    #[error(transparent)]
    Store(#[from] opencnpj_store::Error),
}

impl From<opencnpj_core::varint::Malformed> for Error {
    fn from(_: opencnpj_core::varint::Malformed) -> Self {
        Error::Source("malformed temporary data".into())
    }
}
