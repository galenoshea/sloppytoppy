use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Tabs};

use super::theme::{BRIGHT, MUTED, RED};
use crate::process::{Filter, ProcessEntry};

pub(super) fn render(frame: &mut Frame, area: Rect, entries: &[ProcessEntry], active: Filter) {
    let [brand, tabs] =
        Layout::horizontal([Constraint::Length(26), Constraint::Min(10)]).areas(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("▞▚ ", Style::new().fg(RED).bold()),
            Span::styled("sloppytoppy", Style::new().fg(BRIGHT).bold()),
            Span::styled(
                format!("  v{}", env!("CARGO_PKG_VERSION")),
                Style::new().fg(MUTED),
            ),
        ]))
        .block(Block::new().padding(Padding::new(2, 0, 1, 0))),
        brand,
    );

    let titles = Filter::ALL.map(|f| {
        let n = entries.iter().filter(|e| f.admits(e.tool)).count();
        Line::from(format!("{} {n}", f.title()))
    });
    frame.render_widget(
        Tabs::new(titles)
            .select(active.index())
            .style(Style::new().fg(MUTED))
            .highlight_style(Style::new().fg(Color::Black).bg(RED).bold())
            .padding(" ", " ")
            .divider(" ")
            .block(Block::new().padding(Padding::new(0, 0, 1, 0))),
        tabs,
    );
}
