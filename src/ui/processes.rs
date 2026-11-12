use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Padding, Paragraph, Row, Table, TableState};

use super::format;
use super::theme::{BRIGHT, DEEP, MUTED, RED, TEXT, panel, tool_color, tool_glyph};
use crate::process::ProcessEntry;

/// A fixed-width horizontal meter for a percentage.
fn meter(pct: f32, width: usize) -> Line<'static> {
    let filled = ((pct.clamp(0.0, 100.0) / 100.0) * width as f32).round() as usize;
    let fill = if pct > 80.0 { BRIGHT } else { RED };
    Line::from(vec![
        Span::styled("█".repeat(filled), Style::new().fg(fill)),
        Span::styled("░".repeat(width - filled), Style::new().fg(DEEP)),
    ])
}

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    rows: &[&ProcessEntry],
    state: &mut TableState,
) {
    let body: Vec<Row> = rows
        .iter()
        .map(|e| {
            let tokens = e
                .usage
                .as_ref()
                .map_or_else(|| "-".to_owned(), |u| format::compact(u.tokens.total()));
            Row::new([
                Cell::from(e.pid.to_string()).style(Style::new().fg(MUTED)),
                Cell::from(format!("{} {}", tool_glyph(e.tool), e.tool))
                    .style(Style::new().fg(tool_color(e.tool)).bold()),
                Cell::from(meter(e.cpu, 10)),
                Cell::from(format!("{:>5.1}%", e.cpu)).style(Style::new().fg(TEXT)),
                Cell::from(format!("{:>8}", format::memory(e.memory))).style(Style::new().fg(TEXT)),
                Cell::from(format!("{tokens:>7}")).style(Style::new().fg(TEXT)),
                Cell::from(format::duration(e.uptime)).style(Style::new().fg(MUTED)),
                Cell::from(e.command.clone()).style(Style::new().fg(MUTED)),
            ])
        })
        .collect();

    let table = Table::new(
        body,
        [
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Fill(1),
        ],
    )
    .column_spacing(2)
    .header(
        Row::new([
            "PID", "TOOL", "CPU", "", "MEM", "TOKENS", "UPTIME", "COMMAND",
        ])
        .style(Style::new().fg(RED).bold())
        .bottom_margin(1),
    )
    .row_highlight_style(Style::new().bg(DEEP).fg(Color::White).bold())
    .highlight_symbol("▌ ")
    .block(panel(&format!("Processes ({})", rows.len())).padding(Padding::horizontal(1)));
    frame.render_stateful_widget(table, area, state);

    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new("no claude or codex processes running")
                .style(Style::new().fg(MUTED).italic())
                .alignment(Alignment::Center),
            area.inner(Margin::new(2, 3)),
        );
    }
}
