//! Plain-text snapshot for `--once` and scripting.

use std::io::{self, Write};

use crate::process::Snapshot;
use crate::ui::format;

/// Writes a fixed-width table of `snapshot` to `out`.
///
/// # Errors
/// Propagates write errors.
pub fn write_table(out: &mut impl Write, snapshot: &Snapshot) -> io::Result<()> {
    writeln!(
        out,
        "{:>7}  {:<7} {:>6}  {:>8}  {:>9}  {:<18} COMMAND",
        "PID", "TOOL", "CPU", "MEM", "TOKENS", "MODEL"
    )?;
    for e in &snapshot.entries {
        let usage = e.usage.as_ref();
        writeln!(
            out,
            "{:>7}  {:<7} {:>5.1}%  {:>8}  {:>9}  {:<18} {}",
            e.pid,
            e.tool,
            e.cpu,
            format::memory(e.memory),
            usage.map_or_else(|| "-".to_owned(), |u| format::compact(u.tokens.total())),
            usage.and_then(|u| u.model.as_deref()).unwrap_or("-"),
            e.command,
        )?;
    }
    for w in &snapshot.warnings {
        writeln!(out, "warning: {w}")?;
    }
    Ok(())
}
