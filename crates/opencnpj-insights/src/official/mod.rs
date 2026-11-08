//! Official source-backed indicators, independent from the CNPJ snapshot IR.

pub mod model;
pub mod query;
pub mod registry;
pub mod store;

pub use model::{CellStatus, DatasetManifest, Observation};
pub use query::{Answer, AnswerRow, CategoryFilter, Request, SourceRef, execute, render_pt, unavailable};
pub use registry::{Category, Dataset, Registry, Source, Variable};
pub use store::{INDICATOR_EXTENSION, Published, Store, StoreSet, write_store};

#[cfg(test)]
mod tests;
