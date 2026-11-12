//! Crate-wide error type.

use thiserror::Error;

/// Errors that can abort the application.
#[derive(Debug, Error)]
pub enum Error {
    /// Terminal or other I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience alias used across the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
