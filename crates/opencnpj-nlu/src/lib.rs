//! Brazilian Portuguese question interpretation and dialogue state.

/// Question normalization.
pub mod normalize {}

/// Language pack compilation and loading.
pub mod lang {}

/// Entity spotting and approximate matching.
pub mod spotter {}

/// Time expressions anchored to the data reference date.
pub mod time {}

/// Grammar constructions and request resolution.
pub mod grammar {}

/// Dialogue acts and conversation context.
pub mod context {}

/// Decision policy and confidence signals.
pub mod policy {}
