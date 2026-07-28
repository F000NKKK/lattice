//! Foundational types with no networking semantics of their own.
//!
//! `lattice-core` carries no OS dependency and no networking-specific
//! types — those belong to `lattice-ip` and `lattice-model`. See
//! ARCHITECTURE.md for the full rationale.

mod error;
mod id;

pub use error::{Error, PlatformErrorCode};
pub use id::Id;

pub type Result<T> = core::result::Result<T, Error>;
