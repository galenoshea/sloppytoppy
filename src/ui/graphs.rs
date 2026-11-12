use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Bar, BarChart, BarGroup, Sparkline};

use super::format;
use super::theme::{CRIMSON, RED, TEXT, panel, tool_color, tool_glyph};
use crate::app::App;

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let [cpu, mem, tokens, top] = Layout::horizontal([Constraint::Ratio(1, 4); 4]).areas(area);

    sparkline(
        frame,
        cpu,
        &format!("CPU  {}%", app.cpu.latest()),
        &app.cpu,
        RED,
        Some(100),
    );
    sparkline(
        frame,
        mem,
        &format!(
            "Memory  {}",
            format::memory(app.memory_mb.latest() * 1_048_576)
        ),
        &app.memory_mb,
        CRIMSON,
        None,
    );
    sparkline(
        frame,
        tokens,
        &format!("Tokens  {}", format::compact(app.tokens.latest())),
        &app.tokens,
        Color::Rgb(255, 150, 120),
        None,
    );
    top_cpu(frame, top, app);
}

fn sparkline(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    series: &crate::series::Series,
    color: Color,
    floor: Option<u64>,
) {
    let block = panel(title);
    let data = series.window(usize::from(block.inner(area).width));
    // `floor` keeps a quiet CPU graph from auto-scaling noise up to full height.
    let max = data
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .max(floor.unwrap_or(1));
    frame.render_widget(
        Sparkline::default()
            .data(data)
            .max(max)
            .style(Style::new().fg(color))
            .block(block),
        area,
    );
}

fn top_cpu(frame: &mut Frame, area: Rect, app: &App) {
    let mut top: Vec<_> = app.snapshot.entries.iter().collect();
    top.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let bars: Vec<Bar> = top
        .into_iter()
        .take(5)
        .map(|e| {
            let color = tool_color(e.tool);
            Bar::default()
                .value(e.cpu.round() as u64)
                .label(Line::from(format!("{} {}", tool_glyph(e.tool), e.pid)))
                .text_value(format!("{:.0}%", e.cpu))
                .style(Style::new().fg(color))
                .value_style(Style::new().fg(Color::Black).bg(color).bold())
        })
        .collect();
    frame.render_widget(
        BarChart::default()
            .direction(Direction::Horizontal)
            .bar_width(1)
            .bar_gap(0)
            .label_style(Style::new().fg(TEXT))
            .data(BarGroup::new(bars))
            .block(panel("Top by CPU")),
        area,
    );
}
