//! Token-usage accounting read from the session logs the CLIs write to disk.
//!
//! Each tool has a [`UsageProvider`] that maps a process working directory to
//! its most recent session log and parses it into a [`SessionUsage`].

mod cache;
pub mod claude;
pub mod codex;

use std::iter::Sum;
use std::ops::{Add, AddAssign};
use std::path::{Path, PathBuf};

use thiserror::Error;

pub use claude::ClaudeProvider;
pub use codex::CodexProvider;

/// A breakdown of tokens, normalised across providers.
///
/// `input` excludes cached tokens, so the four billable buckets are disjoint
/// and [`TokenUsage::total`] is a plain sum. `reasoning` is informational: it
/// is already part of `output`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    /// Fresh (uncached) input tokens.
    pub input: u64,
    /// Generated tokens, including reasoning.
    pub output: u64,
    /// Input tokens served from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write: u64,
    /// Subset of `output` spent on reasoning/thinking.
    pub reasoning: u64,
}

impl TokenUsage {
    /// Sum of the four disjoint buckets.
    ///
    /// Log files are untrusted input, so arithmetic saturates instead of
    /// overflowing (which would panic in debug builds and wrap in release).
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    /// Tokens that count towards the context window: everything sent to the
    /// model, i.e. fresh plus cached input.
    #[must_use]
    pub const fn context(&self) -> u64 {
        self.input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }
}

impl Add for TokenUsage {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            input: self.input.saturating_add(rhs.input),
            output: self.output.saturating_add(rhs.output),
            cache_read: self.cache_read.saturating_add(rhs.cache_read),
            cache_write: self.cache_write.saturating_add(rhs.cache_write),
            reasoning: self.reasoning.saturating_add(rhs.reasoning),
        }
    }
}

impl AddAssign for TokenUsage {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sum for TokenUsage {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

impl<'a> Sum<&'a Self> for TokenUsage {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

/// Usage for one session (one conversation log).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionUsage {
    /// Provider session identifier, if the log carries one.
    pub session_id: Option<String>,
    /// Most recently used model.
    pub model: Option<String>,
    /// Cumulative tokens over the session.
    pub tokens: TokenUsage,
    /// Tokens currently occupying the context window.
    pub context_tokens: u64,
    /// Size of the context window, when the log reports it.
    pub context_window: Option<u64>,
    /// Number of model responses / turns observed.
    pub turns: u32,
}

/// Errors raised while locating or reading a session log.
#[derive(Debug, Error)]
pub enum UsageError {
    /// A log file or directory could not be read.
    #[error("reading {}: {source}", path.display())]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl UsageError {
    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// Resolves the token usage of the session a process is running.
pub trait UsageProvider {
    /// Returns usage for the newest session started in `cwd`, or `None` when no
    /// matching session log exists.
    ///
    /// # Errors
    /// Returns [`UsageError`] if a log exists but cannot be read.
    fn usage_for(&mut self, cwd: &Path) -> Result<Option<SessionUsage>, UsageError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_and_sums() {
        let a = TokenUsage {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
            reasoning: 1,
        };
        let b = TokenUsage {
            input: 10,
            ..TokenUsage::default()
        };
        assert_eq!(a.total(), 10);
        let sum: TokenUsage = [a, b].iter().sum();
        assert_eq!(sum.input, 11);
        assert_eq!(sum.total(), 20);
    }

    #[test]
    fn hostile_values_saturate_instead_of_overflowing() {
        let huge = TokenUsage {
            input: u64::MAX,
            output: u64::MAX,
            cache_read: u64::MAX,
            cache_write: u64::MAX,
            reasoning: u64::MAX,
        };
        assert_eq!((huge + huge).total(), u64::MAX);
        assert_eq!(huge.context(), u64::MAX);
    }
}
