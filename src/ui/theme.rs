//! The red palette and shared styles.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};

use crate::process::Tool;

pub(crate) const BRIGHT: Color = Color::Rgb(255, 107, 107);
pub(crate) const RED: Color = Color::Rgb(239, 68, 68);
pub(crate) const CRIMSON: Color = Color::Rgb(190, 18, 60);
pub(crate) const DEEP: Color = Color::Rgb(127, 29, 29);
pub(crate) const EMBER: Color = Color::Rgb(255, 150, 120);
pub(crate) const ROSE: Color = Color::Rgb(251, 191, 191);
pub(crate) const TEXT: Color = Color::Rgb(254, 226, 226);
pub(crate) const MUTED: Color = Color::Rgb(160, 110, 110);

/// Accent colour for a tool.
pub(crate) const fn tool_color(tool: Tool) -> Color {
    match tool {
        Tool::Claude => EMBER,
        Tool::Codex => CRIMSON,
    }
}

/// Marker glyph for a tool.
pub(crate) const fn tool_glyph(tool: Tool) -> &'static str {
    match tool {
        Tool::Claude => "●",
        Tool::Codex => "◆",
    }
}

/// A rounded, titled panel.
pub(crate) fn panel(title: &str) -> Block<'static> {
    let edge = Style::new().fg(DEEP);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(edge)
        .title(Line::from(vec![
            Span::styled(" ", edge),
            Span::styled(title.to_owned(), Style::new().fg(BRIGHT).bold()),
            Span::styled(" ", edge),
        ]))
}

/// A key-cap followed by its description, for the footer.
pub(crate) fn keycap(key: &'static str, desc: &'static str) -> [Span<'static>; 2] {
    [
        Span::styled(
            format!(" {key} "),
            Style::new().fg(Color::Black).bg(RED).bold(),
        ),
        Span::styled(format!(" {desc}  "), Style::new().fg(MUTED)),
    ]
}
