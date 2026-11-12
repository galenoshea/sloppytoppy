use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use super::theme::{BRIGHT, DEEP, MUTED, RED, TEXT};
use crate::app::Confirm;
use crate::process::Kill;

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(row);
    cell
}

pub(super) fn render(frame: &mut Frame, confirm: Confirm) {
    let area = centered(frame.area(), 48, 7);
    let verb = match confirm.how {
        Kill::Terminate => "Terminate (SIGTERM)",
        Kill::Force => "FORCE KILL (SIGKILL)",
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(
                format!("{verb} {} {}?", confirm.tool, confirm.pid),
                Style::new().fg(TEXT).bold(),
            ),
            Line::from(""),
            Line::from(vec![
                Span::styled(" y ", Style::new().fg(Color::Black).bg(RED).bold()),
                Span::styled(" yes   ", Style::new().fg(MUTED)),
                Span::styled(" n ", Style::new().fg(Color::Black).bg(DEEP).bold()),
                Span::styled(" cancel", Style::new().fg(MUTED)),
            ]),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::bordered()
                .border_type(BorderType::Double)
                .border_style(Style::new().fg(RED))
                .title(Span::styled(" confirm ", Style::new().fg(BRIGHT).bold())),
        ),
        area,
    );
}
