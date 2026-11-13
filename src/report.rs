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
            clean(usage.and_then(|u| u.model.as_deref()).unwrap_or("-")),
            clean(&e.command),
        )?;
    }
    for w in &snapshot.warnings {
        writeln!(out, "warning: {}", clean(w))?;
    }
    Ok(())
}

/// Replaces control characters so untrusted text (command lines, log fields)
/// can't smuggle terminal escape sequences into the output.
fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{Pid, ProcessEntry, Tool};

    #[test]
    fn escapes_are_neutralised() {
        let snapshot = Snapshot {
            entries: vec![ProcessEntry {
                pid: Pid::from_u32(1),
                tool: Tool::Claude,
                cpu: 0.0,
                memory: 0,
                uptime: 0,
                command: "claude \x1b]0;pwned\x07\x1b[2J".into(),
                cwd: None,
                usage: None,
            }],
            warnings: vec!["bad \x1b[31m".into()],
        };
        let mut out = Vec::new();
        write_table(&mut out, &snapshot).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains('\x1b') && !text.contains('\x07'));
        assert!(text.contains("claude ?]0;pwned??[2J"));
    }
}
