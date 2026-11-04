//! Data assets, query execution and answer rendering.

/// Reference catalogue loading.
pub mod catalog;

/// Aggregate artifact reading and writing.
pub mod agg;

/// Backend selection for validated requests.
pub mod plan {}

/// Aggregate execution, engine lowering and engine adapters.
pub mod execute {}

/// Answer blocks, provenance and templates.
pub mod render {}
