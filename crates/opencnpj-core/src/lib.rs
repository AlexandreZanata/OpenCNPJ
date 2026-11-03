//! Data model, identifiers and lossless codecs for Receita Federal CNPJ open data.

pub mod cnpj;
pub mod codec;
pub mod model;
pub mod schema;
pub mod text;
pub mod varint;

pub use cnpj::{Cnpj, CnpjError};
pub use model::{Company, Row};
pub use schema::{Kind, RefTable, Table};
