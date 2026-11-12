//! Command-line interface.

use std::time::Duration;

use clap::Parser;

/// View and stop running Claude and Codex processes, with live token usage.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Sampling interval in milliseconds.
    #[arg(short, long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(200..=60_000))]
    pub interval: u64,

    /// Do not read session logs (skips token usage).
    #[arg(long)]
    pub no_usage: bool,

    /// Print a single plain-text snapshot and exit instead of starting the TUI.
    #[arg(long)]
    pub once: bool,
}

impl Cli {
    /// The sampling interval as a [`Duration`].
    #[must_use]
    pub const fn interval(&self) -> Duration {
        Duration::from_millis(self.interval)
    }
}
