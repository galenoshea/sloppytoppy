use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, BorderType, Cell, Clear, Padding, Paragraph, Row,
        Sparkline, Table, TableState, Tabs, Wrap,
    },
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, Uid, UpdateKind};

const REFRESH: Duration = Duration::from_secs(1);
const HISTORY: usize = 300;

// ── Red theme ────────────────────────────────────────────────────────────────
const BRIGHT: Color = Color::Rgb(255, 107, 107);
const RED: Color = Color::Rgb(239, 68, 68);
const CRIMSON: Color = Color::Rgb(190, 18, 60);
const DEEP: Color = Color::Rgb(127, 29, 29);
const EMBER: Color = Color::Rgb(255, 150, 120);
const TEXT: Color = Color::Rgb(254, 226, 226);
const MUTED: Color = Color::Rgb(160, 110, 110);

fn border() -> Style {
    Style::new().fg(DEEP)
}

fn panel(title: &str) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border())
        .title(Line::from(vec![
            Span::styled(" ", border()),
            Span::styled(title.to_string(), Style::new().fg(BRIGHT).bold()),
            Span::styled(" ", border()),
        ]))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Claude,
    Codex,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
            Tool::Codex => "codex",
        }
    }
    fn color(self) -> Color {
        match self {
            Tool::Claude => EMBER,
            Tool::Codex => CRIMSON,
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Tool::Claude => "●",
            Tool::Codex => "◆",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Claude,
    Codex,
}

impl Filter {
    const ALL: [Filter; 3] = [Filter::All, Filter::Claude, Filter::Codex];
    fn matches(self, t: Tool) -> bool {
        match self {
            Filter::All => true,
            Filter::Claude => t == Tool::Claude,
            Filter::Codex => t == Tool::Codex,
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }
}

struct Entry {
    pid: Pid,
    tool: Tool,
    cpu: f32,
    mem: u64,
    runtime: u64,
    cmd: String,
}

/// Classify a process as a Claude/Codex CLI by executable name or by the
/// script it runs (e.g. `node /path/to/bin/codex`). Exact basename matches only.
fn classify(name: &str, argv: &[String]) -> Option<Tool> {
    let base = |s: &str| s.rsplit('/').next().unwrap_or(s).to_ascii_lowercase();
    let check = |s: &str| match base(s).as_str() {
        "claude" => Some(Tool::Claude),
        "codex" => Some(Tool::Codex),
        _ => None,
    };
    if let Some(t) = check(name) {
        return Some(t);
    }
    let first = argv.first().map(String::as_str).unwrap_or("");
    if let Some(t) = check(first) {
        return Some(t);
    }
    if matches!(
        base(first).as_str(),
        "node" | "bun" | "deno" | "python" | "python3"
    ) {
        return argv.get(1).and_then(|s| check(s));
    }
    None
}

struct App {
    sys: System,
    entries: Vec<Entry>,
    table: TableState,
    filter: Filter,
    confirm: Option<(Pid, bool)>, // (pid, force)
    status: String,
    me: Option<Uid>,
    cpu_hist: VecDeque<u64>,
    mem_hist: VecDeque<u64>,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            sys: System::new(),
            entries: vec![],
            table: TableState::default(),
            filter: Filter::All,
            confirm: None,
            status: String::new(),
            me: None,
            cpu_hist: VecDeque::new(),
            mem_hist: VecDeque::new(),
        };
        app.refresh();
        app.me = sysinfo::get_current_pid()
            .ok()
            .and_then(|p| app.sys.process(p))
            .and_then(|p| p.user_id().cloned());
        app
    }

    fn refresh(&mut self) {
        let kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_cmd(UpdateKind::Always)
            .with_user(UpdateKind::OnlyIfNotSet);
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        let self_pid = std::process::id();
        let mut v: Vec<Entry> = self
            .sys
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != self_pid)
            .filter_map(|p| {
                let argv: Vec<String> = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect();
                let tool = classify(&p.name().to_string_lossy(), &argv)?;
                Some(Entry {
                    pid: p.pid(),
                    tool,
                    cpu: p.cpu_usage(),
                    mem: p.memory(),
                    runtime: p.run_time(),
                    cmd: argv.join(" "),
                })
            })
            .collect();
        v.sort_by_key(|e| (e.tool.label(), e.pid.as_u32()));
        self.entries = v;

        let (cpu, mem) = self
            .entries
            .iter()
            .fold((0.0f32, 0u64), |(c, m), e| (c + e.cpu, m + e.mem));
        push(&mut self.cpu_hist, cpu.round() as u64);
        push(&mut self.mem_hist, mem / 1_048_576);
        self.clamp_selection();
    }

    fn visible(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.filter.matches(e.tool))
            .collect()
    }

    fn clamp_selection(&mut self) {
        let n = self.visible().len();
        match (self.table.selected(), n) {
            (_, 0) => self.table.select(None),
            (None, _) => self.table.select(Some(0)),
            (Some(i), n) => self.table.select(Some(i.min(n - 1))),
        }
    }

    fn selected_pid(&self) -> Option<Pid> {
        let i = self.table.selected()?;
        self.visible().get(i).map(|e| e.pid)
    }

    fn kill(&mut self, pid: Pid, force: bool) {
        // Re-verify at kill time: PID must still be a claude/codex process owned by us.
        self.refresh();
        let Some(e) = self.entries.iter().find(|e| e.pid == pid) else {
            self.status = format!("{pid} is gone or no longer a claude/codex process");
            return;
        };
        let label = e.tool.label();
        let Some(p) = self.sys.process(pid) else {
            return;
        };
        if self.me.is_none() || p.user_id() != self.me.as_ref() {
            self.status = format!("refusing: {pid} is not owned by you");
            return;
        }
        let sig = if force { Signal::Kill } else { Signal::Term };
        self.status = match p.kill_with(sig) {
            Some(true) => format!("sent {sig:?} to {label} {pid}"),
            _ => format!("failed to signal {pid}"),
        };
    }
}

fn push(q: &mut VecDeque<u64>, v: u64) {
    if q.len() == HISTORY {
        q.pop_front();
    }
    q.push_back(v);
}

/// Last `w` samples, left-padded with zeros so the graph is right-aligned.
fn window(q: &VecDeque<u64>, w: usize) -> Vec<u64> {
    let skip = q.len().saturating_sub(w);
    let mut out = vec![0; w.saturating_sub(q.len())];
    out.extend(q.iter().skip(skip).copied());
    out
}

fn fmt_dur(s: u64) -> String {
    if s >= 86_400 {
        format!("{}d {:02}h", s / 86_400, s % 86_400 / 3600)
    } else {
        format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    }
}

fn fmt_mem(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}

fn meter(pct: f32, width: usize) -> Line<'static> {
    let filled = ((pct.clamp(0.0, 100.0) / 100.0) * width as f32).round() as usize;
    Line::from(vec![
        Span::styled(
            "█".repeat(filled),
            Style::new().fg(if pct > 80.0 { BRIGHT } else { RED }),
        ),
        Span::styled("░".repeat(width - filled), Style::new().fg(DEEP)),
    ])
}

fn key(k: &'static str, desc: &'static str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {k} "),
            Style::new().fg(Color::Black).bg(RED).bold(),
        ),
        Span::styled(format!(" {desc}  "), Style::new().fg(MUTED)),
    ]
}

fn draw(f: &mut Frame, app: &mut App) {
    let [header, graphs, main, detail, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(9),
        Constraint::Min(5),
        Constraint::Length(4),
        Constraint::Length(2),
    ])
    .areas(f.area());

    // Header: brand + tabs
    let [brand, tabs] =
        Layout::horizontal([Constraint::Length(26), Constraint::Min(10)]).areas(header);
    f.render_widget(
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
    let count = |flt: Filter| app.entries.iter().filter(|e| flt.matches(e.tool)).count();
    let titles = Filter::ALL.map(|flt| {
        let name = match flt {
            Filter::All => "All",
            Filter::Claude => "Claude",
            Filter::Codex => "Codex",
        };
        Line::from(format!("{name} {}", count(flt)))
    });
    f.render_widget(
        Tabs::new(titles)
            .select(app.filter.index())
            .style(Style::new().fg(MUTED))
            .highlight_style(Style::new().fg(Color::Black).bg(RED).bold())
            .padding(" ", " ")
            .divider(" ")
            .block(Block::new().padding(Padding::new(0, 0, 1, 0))),
        tabs,
    );

    // Graphs: CPU, memory, top processes
    let [cpu_a, mem_a, top_a] = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(34),
    ])
    .areas(graphs);

    let cpu_now = app.cpu_hist.back().copied().unwrap_or(0);
    let cpu_block = panel(&format!("CPU  {cpu_now}%"));
    let w = cpu_block.inner(cpu_a).width as usize;
    let data = window(&app.cpu_hist, w);
    let cpu_max = data.iter().copied().max().unwrap_or(0).max(100);
    f.render_widget(
        Sparkline::default()
            .data(data)
            .max(cpu_max)
            .style(Style::new().fg(RED))
            .block(cpu_block),
        cpu_a,
    );

    let mem_now = app.mem_hist.back().copied().unwrap_or(0);
    let mem_block = panel(&format!("Memory  {}", fmt_mem(mem_now * 1_048_576)));
    let w = mem_block.inner(mem_a).width as usize;
    f.render_widget(
        Sparkline::default()
            .data(window(&app.mem_hist, w))
            .style(Style::new().fg(CRIMSON))
            .block(mem_block),
        mem_a,
    );

    let mut top: Vec<&Entry> = app.entries.iter().collect();
    top.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let bars: Vec<Bar> = top
        .iter()
        .take(5)
        .map(|e| {
            Bar::default()
                .value(e.cpu.round() as u64)
                .label(Line::from(format!("{} {}", e.tool.glyph(), e.pid)))
                .text_value(format!("{:.0}%", e.cpu))
                .style(Style::new().fg(e.tool.color()))
                .value_style(Style::new().fg(Color::Black).bg(e.tool.color()).bold())
        })
        .collect();
    f.render_widget(
        BarChart::default()
            .direction(Direction::Horizontal)
            .bar_width(1)
            .bar_gap(0)
            .label_style(Style::new().fg(TEXT))
            .data(BarGroup::new(bars))
            .block(panel("Top by CPU")),
        top_a,
    );

    // Process table
    let flt = app.filter;
    let visible: Vec<&Entry> = app.entries.iter().filter(|e| flt.matches(e.tool)).collect();
    let rows: Vec<Row> = visible
        .iter()
        .map(|e| {
            Row::new([
                Cell::from(e.pid.to_string()).style(Style::new().fg(MUTED)),
                Cell::from(format!("{} {}", e.tool.glyph(), e.tool.label()))
                    .style(Style::new().fg(e.tool.color()).bold()),
                Cell::from(meter(e.cpu, 10)),
                Cell::from(format!("{:>5.1}%", e.cpu)).style(Style::new().fg(TEXT)),
                Cell::from(format!("{:>8}", fmt_mem(e.mem))).style(Style::new().fg(TEXT)),
                Cell::from(fmt_dur(e.runtime)).style(Style::new().fg(MUTED)),
                Cell::from(e.cmd.clone()).style(Style::new().fg(MUTED)),
            ])
        })
        .collect();
    let empty = rows.is_empty();
    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(11),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Fill(1),
        ],
    )
    .column_spacing(2)
    .header(
        Row::new(["PID", "TOOL", "CPU", "", "MEM", "UPTIME", "COMMAND"])
            .style(Style::new().fg(RED).bold())
            .bottom_margin(1),
    )
    .row_highlight_style(Style::new().bg(DEEP).fg(Color::White).bold())
    .highlight_symbol("▌ ")
    .block(panel(&format!("Processes ({})", visible.len())).padding(Padding::horizontal(1)));
    let mut tstate = std::mem::take(&mut app.table);
    f.render_stateful_widget(table, main, &mut tstate);
    app.table = tstate;
    if empty {
        let inner = main.inner(ratatui::layout::Margin::new(2, 3));
        f.render_widget(
            Paragraph::new("no claude or codex processes running")
                .style(Style::new().fg(MUTED).italic())
                .alignment(Alignment::Center),
            inner,
        );
    }

    // Detail
    let sel = app.table.selected().and_then(|i| visible.get(i));
    let detail_line = match sel {
        Some(e) => Line::from(vec![
            Span::styled(
                format!("{} ", e.tool.glyph()),
                Style::new().fg(e.tool.color()),
            ),
            Span::styled(e.cmd.clone(), Style::new().fg(TEXT)),
        ]),
        None => Line::styled("nothing selected", Style::new().fg(MUTED)),
    };
    f.render_widget(
        Paragraph::new(detail_line)
            .wrap(Wrap { trim: true })
            .block(panel("Command").padding(Padding::horizontal(1))),
        detail,
    );

    // Footer
    let mut keys: Vec<Span> = vec![];
    for (k, d) in [
        ("↑↓", "select"),
        ("⇥", "filter"),
        ("k", "terminate"),
        ("K", "kill -9"),
        ("r", "refresh"),
        ("q", "quit"),
    ] {
        keys.extend(key(k, d));
    }
    f.render_widget(
        Paragraph::new(vec![
            Line::from(keys),
            Line::styled(format!(" {}", app.status), Style::new().fg(BRIGHT).italic()),
        ]),
        footer,
    );

    // Confirm modal
    if let Some((pid, force)) = app.confirm {
        let tool = app
            .entries
            .iter()
            .find(|e| e.pid == pid)
            .map(|e| e.tool.label())
            .unwrap_or("process");
        let area = center(f.area(), 48, 7);
        f.render_widget(Clear, area);
        let verb = if force {
            "FORCE KILL (SIGKILL)"
        } else {
            "Terminate (SIGTERM)"
        };
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::styled(
                    format!("{verb} {tool} {pid}?"),
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
}

fn center(area: Rect, w: u16, h: u16) -> Rect {
    let [v] = Layout::vertical([Constraint::Length(h)])
        .flex(Flex::Center)
        .areas(area);
    let [r] = Layout::horizontal([Constraint::Length(w)])
        .flex(Flex::Center)
        .areas(v);
    r
}

fn run(mut term: DefaultTerminal) -> std::io::Result<()> {
    let mut app = App::new();
    let mut last = Instant::now();
    loop {
        term.draw(|f| draw(f, &mut app))?;

        let timeout = REFRESH.saturating_sub(last.elapsed());
        if event::poll(timeout)?
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                return Ok(());
            }
            if let Some((pid, force)) = app.confirm.take() {
                if matches!(k.code, KeyCode::Char('y' | 'Y')) {
                    app.kill(pid, force);
                } else {
                    app.status = "cancelled".into();
                }
                continue;
            }
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.table.select_next(),
                KeyCode::Up => app.table.select_previous(),
                KeyCode::Tab | KeyCode::Right => {
                    app.filter = Filter::ALL[(app.filter.index() + 1) % 3];
                    app.clamp_selection();
                }
                KeyCode::BackTab | KeyCode::Left => {
                    app.filter = Filter::ALL[(app.filter.index() + 2) % 3];
                    app.clamp_selection();
                }
                KeyCode::Char('r') => app.refresh(),
                KeyCode::Char(c @ ('k' | 'K')) => {
                    if let Some(pid) = app.selected_pid() {
                        app.confirm = Some((pid, c == 'K'));
                    }
                }
                _ => {}
            }
            app.clamp_selection();
        }
        if last.elapsed() >= REFRESH {
            app.refresh();
            last = Instant::now();
        }
    }
}

fn main() -> std::io::Result<()> {
    let term = ratatui::init();
    let res = run(term);
    ratatui::restore();
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn fake(pid: u32, tool: Tool, cpu: f32) -> Entry {
        Entry {
            pid: Pid::from_u32(pid),
            tool,
            cpu,
            mem: 300 * 1_048_576,
            runtime: 4000,
            cmd: format!("/usr/local/bin/{} --resume", tool.label()),
        }
    }

    #[test]
    fn classify_matches_exact_names_only() {
        assert!(classify("claude", &[]).is_some());
        assert!(classify("node", &["node".into(), "/x/bin/codex".into()]).is_some());
        assert!(classify("claude-helper", &[]).is_none());
        assert!(classify("node", &["node".into(), "/x/server.js".into()]).is_none());
    }

    #[test]
    fn renders_without_panic() {
        let mut app = App::new();
        app.entries = vec![fake(101, Tool::Claude, 42.0), fake(202, Tool::Codex, 85.0)];
        for i in 0..40 {
            push(&mut app.cpu_hist, 20 + (i * 7) % 60);
            push(&mut app.mem_hist, 500 + i * 10);
        }
        app.clamp_selection();
        app.confirm = Some((Pid::from_u32(101), false));
        let mut t = Terminal::new(TestBackend::new(120, 38)).unwrap();
        t.draw(|f| draw(f, &mut app)).unwrap();
        println!("{}", t.backend());
    }
}
