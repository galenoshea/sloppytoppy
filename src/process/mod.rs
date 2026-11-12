//! Discovering, sampling and signalling `claude` / `codex` processes.

mod kill;
mod sampler;

use std::fmt;
use std::path::{Path, PathBuf};

pub use kill::{Kill, KillError, signal};
pub use sampler::{Sampler, SamplerHandle, Snapshot, spawn};
pub use sysinfo::Pid;

use crate::usage::SessionUsage;

/// Which agent CLI a process belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// Claude Code.
    Claude,
    /// The Codex CLI.
    Codex,
}

impl Tool {
    /// Lower-case executable name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// Classifies a process by executable name or, for interpreter-launched
    /// scripts (`node /path/to/codex`), by the script name. Only exact
    /// file-stem matches count, so `claude-helper` or `server.js` never match.
    #[must_use]
    pub fn classify(name: &str, argv: &[String]) -> Option<Self> {
        fn stem(s: &str) -> Option<String> {
            Path::new(s)
                .file_stem()?
                .to_str()
                .map(str::to_ascii_lowercase)
        }
        fn from_stem(s: &str) -> Option<Tool> {
            match stem(s)?.as_str() {
                "claude" => Some(Tool::Claude),
                "codex" => Some(Tool::Codex),
                _ => None,
            }
        }

        if let Some(tool) = from_stem(name) {
            return Some(tool);
        }
        let first = argv.first()?;
        if let Some(tool) = from_stem(first) {
            return Some(tool);
        }
        let interpreter = stem(first)?;
        if matches!(
            interpreter.as_str(),
            "node" | "bun" | "deno" | "python" | "python3"
        ) {
            return argv.get(1).and_then(|script| from_stem(script));
        }
        None
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// One running agent process.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    /// OS process id.
    pub pid: Pid,
    /// Agent type.
    pub tool: Tool,
    /// CPU usage in percent (may exceed 100 on multi-core).
    pub cpu: f32,
    /// Resident memory in bytes.
    pub memory: u64,
    /// Seconds since the process started.
    pub uptime: u64,
    /// Full command line.
    pub command: String,
    /// Working directory, when visible to us.
    pub cwd: Option<PathBuf>,
    /// Token usage of the session this process is running, when resolvable.
    pub usage: Option<SessionUsage>,
}

/// Which tools the process list is restricted to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Filter {
    /// Everything.
    #[default]
    All,
    /// Claude only.
    Claude,
    /// Codex only.
    Codex,
}

impl Filter {
    /// Every filter, in tab order.
    pub const ALL: [Self; 3] = [Self::All, Self::Claude, Self::Codex];

    /// Whether `tool` passes this filter.
    #[must_use]
    pub fn admits(self, tool: Tool) -> bool {
        match self {
            Self::All => true,
            Self::Claude => tool == Tool::Claude,
            Self::Codex => tool == Tool::Codex,
        }
    }

    /// Position in [`Filter::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|f| *f == self)
            .unwrap_or_default()
    }

    /// The next filter, wrapping.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    /// The previous filter, wrapping.
    #[must_use]
    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Tab title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Claude => "Claude",
            Self::Codex => "Codex",
        }
    }

    /// Entries passing this filter.
    #[must_use]
    pub fn apply(self, entries: &[ProcessEntry]) -> Vec<&ProcessEntry> {
        entries.iter().filter(|e| self.admits(e.tool)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn classifies_direct_and_interpreted() {
        assert_eq!(Tool::classify("claude", &[]), Some(Tool::Claude));
        assert_eq!(
            Tool::classify("2.1.3", &argv(&["claude", "--resume"])),
            Some(Tool::Claude)
        );
        assert_eq!(
            Tool::classify("node", &argv(&["node", "/x/bin/codex"])),
            Some(Tool::Codex)
        );
        assert_eq!(Tool::classify("Codex.EXE", &[]), Some(Tool::Codex));
    }

    #[test]
    fn rejects_lookalikes() {
        assert_eq!(Tool::classify("claude-helper", &[]), None);
        assert_eq!(
            Tool::classify("node", &argv(&["node", "/x/server.js"])),
            None
        );
        assert_eq!(Tool::classify("vim", &argv(&["vim", "codex"])), None);
    }

    #[test]
    fn filter_cycles() {
        assert_eq!(Filter::All.next(), Filter::Claude);
        assert_eq!(Filter::All.prev(), Filter::Codex);
        assert!(Filter::Codex.admits(Tool::Codex) && !Filter::Codex.admits(Tool::Claude));
    }
}
