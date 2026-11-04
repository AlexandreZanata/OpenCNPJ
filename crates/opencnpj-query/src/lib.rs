//! Typed requests shared by the interpreter and execution backends.

/// Query types and their canonical form.
pub mod ir;

/// Versioned binary encoding and cache keys.
pub mod codec;

/// Validation against execution capabilities.
pub mod validate {}

/// Textual representation of requests.
pub mod print;
