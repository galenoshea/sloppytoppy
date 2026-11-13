//! Application state and its update logic. No rendering happens here.

use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};
use ratatui::widgets::TableState;

use crate::input::{Action, map_key};
use crate::process::{Filter, Kill, Pid, ProcessEntry, SamplerHandle, Snapshot, Tool, signal};
use crate::series::Series;
use crate::ui;

/// Samples kept for the sparklines.
const HISTORY: usize = 300;
/// How often the event loop wakes to check for new snapshots.
const TICK: Duration = Duration::from_millis(100);

/// A pending confirmation to signal a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirm {
    /// Target process.
    pub pid: Pid,
    /// Which tool it was when the dialog opened.
    pub tool: Tool,
    /// Signal to send.
    pub how: Kill,
}

/// All mutable UI state.
#[derive(Debug)]
pub struct App {
    pub(crate) snapshot: Snapshot,
    pub(crate) table: TableState,
    pub(crate) filter: Filter,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) status: String,
    pub(crate) cpu: Series,
    pub(crate) memory_mb: Series,
    pub(crate) tokens: Series,
    should_quit: bool,
    sampler: Option<SamplerHandle>,
}

impl App {
    /// Creates the app, optionally attached to a live sampler.
    #[must_use]
    pub fn new(sampler: Option<SamplerHandle>) -> Self {
        Self {
            snapshot: Snapshot::default(),
            table: TableState::default(),
            filter: Filter::default(),
            confirm: None,
            status: String::new(),
            cpu: Series::new(HISTORY),
            memory_mb: Series::new(HISTORY),
            tokens: Series::new(HISTORY),
            should_quit: false,
            sampler,
        }
    }

    /// Runs the event loop until the user quits.
    ///
    /// # Errors
    /// Propagates terminal I/O errors.
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let mut dirty = true;
        while !self.should_quit {
            if dirty {
                terminal.draw(|frame| ui::render(frame, self))?;
                dirty = false;
            }
            if event::poll(TICK)?
                && let Event::Key(key) = event::read()?
                && let Some(action) = map_key(key, self.confirm.is_some())
            {
                self.update(action);
                dirty = true;
            }
            dirty |= self.drain_snapshots();
        }
        Ok(())
    }

    /// Pulls every pending snapshot; applies the newest. Returns whether any arrived.
    fn drain_snapshots(&mut self) -> bool {
        let latest = self
            .sampler
            .as_ref()
            .and_then(|s| s.snapshots.try_iter().last());
        latest
            .map(|snapshot| self.apply_snapshot(snapshot))
            .is_some()
    }

    /// Installs a new snapshot and extends the history series.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        let cpu: f32 = snapshot.entries.iter().map(|e| e.cpu).sum();
        let mem = snapshot
            .entries
            .iter()
            .fold(0u64, |acc, e| acc.saturating_add(e.memory));
        let tokens = snapshot
            .entries
            .iter()
            .filter_map(|e| e.usage.as_ref())
            .fold(0u64, |acc, u| acc.saturating_add(u.tokens.total()));
        self.cpu.push(cpu.round() as u64);
        self.memory_mb.push(mem / 1_048_576);
        self.tokens.push(tokens);
        self.snapshot = snapshot;
        self.clamp_selection();
    }

    /// Processes passing the active filter.
    #[must_use]
    pub fn visible(&self) -> Vec<&ProcessEntry> {
        self.filter.apply(&self.snapshot.entries)
    }

    /// The highlighted process, if any.
    #[must_use]
    pub fn selected(&self) -> Option<&ProcessEntry> {
        let i = self.table.selected()?;
        self.visible().get(i).copied()
    }

    fn clamp_selection(&mut self) {
        let len = self.visible().len();
        let next = match (self.table.selected(), len) {
            (_, 0) => None,
            (None, _) => Some(0),
            (Some(i), n) => Some(i.min(n - 1)),
        };
        self.table.select(next);
    }

    /// Applies a user action.
    pub fn update(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Next => self.table.select_next(),
            Action::Prev => self.table.select_previous(),
            Action::First => self.table.select_first(),
            Action::Last => self.table.select_last(),
            Action::NextFilter => self.set_filter(self.filter.next()),
            Action::PrevFilter => self.set_filter(self.filter.prev()),
            Action::Refresh => {
                if let Some(s) = &self.sampler {
                    s.request_refresh();
                }
            }
            Action::AskKill(how) => {
                self.confirm = self.selected().map(|e| Confirm {
                    pid: e.pid,
                    tool: e.tool,
                    how,
                });
            }
            Action::Confirm => {
                if let Some(c) = self.confirm.take() {
                    self.status = match signal(c.pid, c.how) {
                        Ok(tool) => format!("sent {:?} to {tool} {}", c.how, c.pid),
                        Err(e) => e.to_string(),
                    };
                    self.update(Action::Refresh);
                }
            }
            Action::Cancel => {
                self.confirm = None;
                self.status = "cancelled".into();
            }
        }
        self.clamp_selection();
    }

    fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
        self.table.select(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::{SessionUsage, TokenUsage};

    fn entry(pid: u32, tool: Tool) -> ProcessEntry {
        ProcessEntry {
            pid: Pid::from_u32(pid),
            tool,
            cpu: 10.0,
            memory: 64 * 1_048_576,
            uptime: 60,
            command: tool.label().into(),
            cwd: None,
            usage: Some(SessionUsage {
                tokens: TokenUsage {
                    input: 5,
                    output: 5,
                    ..TokenUsage::default()
                },
                ..SessionUsage::default()
            }),
        }
    }

    fn app() -> App {
        let mut app = App::new(None);
        app.apply_snapshot(Snapshot {
            entries: vec![
                entry(1, Tool::Claude),
                entry(2, Tool::Codex),
                entry(3, Tool::Codex),
            ],
            warnings: vec![],
        });
        app
    }

    #[test]
    fn snapshot_extends_history_and_selects_first() {
        let app = app();
        assert_eq!(app.cpu.latest(), 30);
        assert_eq!(app.memory_mb.latest(), 192);
        assert_eq!(app.tokens.latest(), 30);
        assert_eq!(app.table.selected(), Some(0));
    }

    #[test]
    fn filter_changes_visible_rows_and_clamps() {
        let mut app = app();
        app.update(Action::Last);
        assert_eq!(app.selected().unwrap().pid, Pid::from_u32(3));
        app.update(Action::NextFilter); // Claude: one row
        assert_eq!(app.visible().len(), 1);
        assert_eq!(app.table.selected(), Some(0));
    }

    #[test]
    fn kill_requires_confirmation_and_cancel_clears_it() {
        let mut app = app();
        app.update(Action::AskKill(Kill::Terminate));
        assert_eq!(app.confirm.map(|c| c.pid), Some(Pid::from_u32(1)));
        app.update(Action::Cancel);
        assert!(app.confirm.is_none());
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = app();
        app.update(Action::Quit);
        assert!(app.should_quit);
    }
}
