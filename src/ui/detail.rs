use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Padding, Paragraph, Wrap};

use super::format::compact;
use super::theme::{
    BRIGHT, CRIMSON, DEEP, EMBER, MUTED, RED, ROSE, TEXT, panel, tool_color, tool_glyph,
};
use crate::process::ProcessEntry;
use crate::usage::{SessionUsage, TokenUsage};

pub(super) fn render(frame: &mut Frame, area: Rect, selected: Option<&ProcessEntry>) {
    let [command, usage] =
        Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)]).areas(area);
    render_command(frame, command, selected);
    render_usage(frame, usage, selected);
}

fn render_command(frame: &mut Frame, area: Rect, selected: Option<&ProcessEntry>) {
    let mut lines = Vec::new();
    match selected {
        Some(e) => {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", tool_glyph(e.tool)),
                    Style::new().fg(tool_color(e.tool)),
                ),
                Span::styled(e.command.clone(), Style::new().fg(TEXT)),
            ]));
            if let Some(cwd) = &e.cwd {
                lines.push(Line::styled(
                    format!("in {}", cwd.display()),
                    Style::new().fg(MUTED),
                ));
            }
        }
        None => lines.push(Line::styled("nothing selected", Style::new().fg(MUTED))),
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel("Command").padding(Padding::horizontal(1))),
        area,
    );
}

fn render_usage(frame: &mut Frame, area: Rect, selected: Option<&ProcessEntry>) {
    let block = panel("Token usage").padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(usage) = selected.and_then(|e| e.usage.as_ref()) else {
        let note = if selected.is_some() {
            "no session log found for this process"
        } else {
            "select a process"
        };
        frame.render_widget(
            Paragraph::new(Line::styled(note, Style::new().fg(MUTED).italic())),
            inner,
        );
        return;
    };

    let [head, bar, legend, ctx] = Layout::vertical([Constraint::Length(1); 4]).areas(inner);
    frame.render_widget(Paragraph::new(heading(usage)), head);
    frame.render_widget(
        Paragraph::new(stacked_bar(&usage.tokens, usize::from(bar.width))),
        bar,
    );
    frame.render_widget(Paragraph::new(legend_line(&usage.tokens)), legend);
    context_gauge(frame, ctx, usage);
}

fn heading(u: &SessionUsage) -> Line<'static> {
    Line::from(vec![
        Span::styled(compact(u.tokens.total()), Style::new().fg(BRIGHT).bold()),
        Span::styled(" tokens", Style::new().fg(MUTED)),
        Span::styled(
            format!("  ·  {}", u.model.as_deref().unwrap_or("unknown model")),
            Style::new().fg(TEXT),
        ),
        Span::styled(format!("  ·  {} turns", u.turns), Style::new().fg(MUTED)),
    ])
}

/// `(label, tokens, colour)` for the four disjoint buckets.
fn buckets(t: &TokenUsage) -> [(&'static str, u64, Color); 4] {
    [
        ("in", t.input, EMBER),
        ("out", t.output, RED),
        ("cache-r", t.cache_read, CRIMSON),
        ("cache-w", t.cache_write, ROSE),
    ]
}

/// A single-row bar split proportionally between the buckets.
fn stacked_bar(t: &TokenUsage, width: usize) -> Line<'static> {
    let total = t.total();
    if total == 0 || width == 0 {
        return Line::styled("░".repeat(width), Style::new().fg(DEEP));
    }
    let mut spans = Vec::new();
    let mut used = 0;
    for (i, (_, n, color)) in buckets(t).into_iter().enumerate() {
        // The last non-empty bucket absorbs rounding so the bar is always full.
        let cells = if i == 3 {
            width - used
        } else {
            (n as f64 / total as f64 * width as f64).round() as usize
        };
        let cells = cells.min(width - used);
        used += cells;
        spans.push(Span::styled("█".repeat(cells), Style::new().fg(color)));
    }
    Line::from(spans)
}

fn legend_line(t: &TokenUsage) -> Line<'static> {
    let mut spans = Vec::new();
    for (label, n, color) in buckets(t) {
        spans.push(Span::styled("■ ", Style::new().fg(color)));
        spans.push(Span::styled(
            format!("{label} {}  ", compact(n)),
            Style::new().fg(MUTED),
        ));
    }
    Line::from(spans)
}

fn context_gauge(frame: &mut Frame, area: Rect, u: &SessionUsage) {
    match u.context_window.filter(|w| *w > 0) {
        Some(window) => {
            let ratio = (u.context_tokens as f64 / window as f64).clamp(0.0, 1.0);
            frame.render_widget(
                Gauge::default()
                    .ratio(ratio)
                    .use_unicode(true)
                    .gauge_style(
                        Style::new()
                            .fg(if ratio > 0.8 { BRIGHT } else { RED })
                            .bg(DEEP),
                    )
                    .label(format!(
                        "context {} / {} ({:.0}%)",
                        compact(u.context_tokens),
                        compact(window),
                        ratio * 100.0
                    )),
                area,
            );
        }
        None => frame.render_widget(
            Paragraph::new(Line::styled(
                format!("context {}", compact(u.context_tokens)),
                Style::new().fg(MUTED),
            )),
            area,
        ),
    }
}
