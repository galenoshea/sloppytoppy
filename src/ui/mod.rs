//! Stateless rendering: `App` in, pixels out.

mod detail;
pub mod format;
mod graphs;
mod header;
mod modal;
mod processes;
mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::App;
use theme::{BRIGHT, keycap};

/// Draws the whole interface.
pub fn render(frame: &mut Frame, app: &mut App) {
    let [header, graphs, main, detail, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(9),
        Constraint::Min(5),
        Constraint::Length(6),
        Constraint::Length(2),
    ])
    .areas(frame.area());

    header::render(frame, header, &app.snapshot.entries, app.filter);
    graphs::render(frame, graphs, app);

    // Borrow the table state mutably while the rows borrow the snapshot.
    let App {
        snapshot,
        filter,
        table,
        confirm,
        status,
        ..
    } = &mut *app;
    let rows = filter.apply(&snapshot.entries);
    processes::render(frame, main, &rows, table);
    let selected = table.selected().and_then(|i| rows.get(i)).copied();
    detail::render(frame, detail, selected);

    let hints: Vec<_> = [
        ("↑↓", "select"),
        ("⇥", "filter"),
        ("k", "terminate"),
        ("K", "kill -9"),
        ("r", "refresh"),
        ("q", "quit"),
    ]
    .into_iter()
    .flat_map(|(k, d)| keycap(k, d))
    .collect();
    let message = if status.is_empty() {
        snapshot.warnings.first().cloned().unwrap_or_default()
    } else {
        status.clone()
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(hints),
            Line::styled(format!(" {message}"), Style::new().fg(BRIGHT).italic()),
        ]),
        footer,
    );

    if let Some(confirm) = *confirm {
        modal::render(frame, confirm);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::process::{Kill, Pid, ProcessEntry, Snapshot, Tool};
    use crate::usage::{SessionUsage, TokenUsage};

    fn entry(pid: u32, tool: Tool, with_usage: bool) -> ProcessEntry {
        ProcessEntry {
            pid: Pid::from_u32(pid),
            tool,
            cpu: 42.0,
            memory: 300 * 1_048_576,
            uptime: 4_000,
            command: format!("/usr/local/bin/{tool} --resume"),
            cwd: Some("/work/app".into()),
            usage: with_usage.then(|| SessionUsage {
                model: Some("claude-sonnet-5-5".into()),
                tokens: TokenUsage {
                    input: 1_200,
                    output: 8_000,
                    cache_read: 2_400_000,
                    cache_write: 90_000,
                    reasoning: 100,
                },
                context_tokens: 60_000,
                context_window: Some(200_000),
                turns: 12,
                ..SessionUsage::default()
            }),
        }
    }

    fn draw(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(130, 40)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn renders_usage_and_rows() {
        let mut app = App::new(None);
        for _ in 0..30 {
            app.apply_snapshot(Snapshot {
                entries: vec![
                    entry(101, Tool::Claude, true),
                    entry(202, Tool::Codex, false),
                ],
                warnings: vec![],
            });
        }
        let screen = draw(&mut app);
        assert!(screen.contains("Token usage"));
        assert!(screen.contains("2.5M tokens"));
        assert!(screen.contains("claude-sonnet-5-5"));
        assert!(screen.contains("context 60.0k / 200.0k (30%)"));
        assert!(screen.contains("202"));
    }

    #[test]
    fn renders_confirm_modal_and_empty_state() {
        let mut app = App::new(None);
        assert!(draw(&mut app).contains("no claude or codex processes running"));

        app.apply_snapshot(Snapshot {
            entries: vec![entry(7, Tool::Codex, false)],
            warnings: vec![],
        });
        app.update(crate::input::Action::AskKill(Kill::Force));
        let screen = draw(&mut app);
        assert!(screen.contains("FORCE KILL (SIGKILL) codex 7?"));
    }
}
