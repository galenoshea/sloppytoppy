use std::time::{Duration, Instant};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Cell, Paragraph, Row, Table, TableState},
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, Uid, UpdateKind};

const REFRESH: Duration = Duration::from_secs(2);

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
    // Interpreter-launched: node/bun/python <script>
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
    confirm: Option<(Pid, bool)>, // (pid, force)
    status: String,
    me: Option<Uid>,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            sys: System::new(),
            entries: vec![],
            table: TableState::default(),
            confirm: None,
            status: String::new(),
            me: None,
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
        let sel = self.table.selected();
        self.entries = v;
        match (sel, self.entries.len()) {
            (_, 0) => self.table.select(None),
            (None, _) => self.table.select(Some(0)),
            (Some(i), n) => self.table.select(Some(i.min(n - 1))),
        }
    }

    fn selected(&self) -> Option<&Entry> {
        self.table.selected().and_then(|i| self.entries.get(i))
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

fn fmt_dur(s: u64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

fn run(mut term: DefaultTerminal) -> std::io::Result<()> {
    let mut app = App::new();
    let mut last = Instant::now();
    loop {
        term.draw(|f| {
            let [main, foot] =
                Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(f.area());
            let rows = app.entries.iter().map(|e| {
                let color = match e.tool {
                    Tool::Claude => Color::Rgb(217, 119, 87),
                    Tool::Codex => Color::Green,
                };
                Row::new([
                    Cell::from(e.pid.to_string()),
                    Cell::from(e.tool.label()).style(Style::new().fg(color)),
                    Cell::from(format!("{:.1}%", e.cpu)),
                    Cell::from(format!("{} MB", e.mem / 1_048_576)),
                    Cell::from(fmt_dur(e.runtime)),
                    Cell::from(e.cmd.clone()),
                ])
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(7),
                    Constraint::Length(9),
                    Constraint::Length(9),
                    Constraint::Fill(1),
                ],
            )
            .header(
                Row::new(["PID", "TOOL", "CPU", "MEM", "UP", "COMMAND"])
                    .style(Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            )
            .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .block(Block::bordered().title(format!(
                " sloppytoppy — {} claude/codex processes ",
                app.entries.len()
            )));
            f.render_stateful_widget(table, main, &mut app.table);

            let help = match app.confirm {
                Some((pid, force)) => Line::styled(
                    format!(
                        "{} {pid}? (y/n)",
                        if force { "SIGKILL" } else { "Terminate" }
                    ),
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                None => Line::from("↑/↓ select · k terminate · K force kill · r refresh · q quit"),
            };
            f.render_widget(
                Paragraph::new(vec![help, Line::from(app.status.as_str())]),
                foot,
            );
        })?;

        let timeout = REFRESH.saturating_sub(last.elapsed());
        if event::poll(timeout)?
            && let Event::Key(k) = event::read()?
        {
            if k.kind != KeyEventKind::Press {
                continue;
            }
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
                KeyCode::Char('r') => app.refresh(),
                KeyCode::Char(c @ ('k' | 'K')) => {
                    if let Some(e) = app.selected() {
                        app.confirm = Some((e.pid, c == 'K'));
                    }
                }
                _ => {}
            }
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
