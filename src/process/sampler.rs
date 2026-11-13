//! Background sampling: process table + token usage, off the UI thread.

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::{ProcessEntry, Tool, display_command};
use crate::usage::{ClaudeProvider, CodexProvider, UsageProvider};

/// One point-in-time view of every agent process.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Processes sorted by tool then PID.
    pub entries: Vec<ProcessEntry>,
    /// Non-fatal problems met while reading usage logs.
    pub warnings: Vec<String>,
}

/// Owns the `sysinfo` state and the usage providers.
#[derive(Debug)]
pub struct Sampler {
    system: System,
    claude: Option<ClaudeProvider>,
    codex: Option<CodexProvider>,
}

impl Sampler {
    /// Creates a sampler. With `with_usage == false` no session logs are read.
    #[must_use]
    pub fn new(with_usage: bool) -> Self {
        Self {
            system: System::new(),
            claude: with_usage.then(ClaudeProvider::from_env).flatten(),
            codex: with_usage.then(CodexProvider::from_env).flatten(),
        }
    }

    /// Refreshes the process table and resolves usage for each agent process.
    pub fn sample(&mut self) -> Snapshot {
        let kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::OnlyIfNotSet);
        self.system
            .refresh_processes_specifics(ProcessesToUpdate::All, true, kind);

        let own_pid = std::process::id();
        let mut warnings = Vec::new();
        let mut entries: Vec<ProcessEntry> = self
            .system
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != own_pid)
            .filter_map(|p| {
                let argv: Vec<String> = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect();
                let tool = Tool::classify(&p.name().to_string_lossy(), &argv)?;
                Some(ProcessEntry {
                    pid: p.pid(),
                    tool,
                    cpu: p.cpu_usage(),
                    memory: p.memory(),
                    uptime: p.run_time(),
                    command: display_command(&argv),
                    cwd: p.cwd().map(ToOwned::to_owned),
                    usage: None,
                })
            })
            .collect();

        for entry in &mut entries {
            let Some(cwd) = entry.cwd.as_deref() else {
                continue;
            };
            let result = match entry.tool {
                Tool::Claude => self.claude.as_mut().map(|p| p.usage_for(cwd)),
                Tool::Codex => self.codex.as_mut().map(|p| p.usage_for(cwd)),
            };
            match result {
                Some(Ok(usage)) => entry.usage = usage,
                Some(Err(e)) => warnings.push(e.to_string()),
                None => {}
            }
        }

        entries.sort_by_key(|e| (e.tool.label(), e.pid.as_u32()));
        Snapshot { entries, warnings }
    }
}

/// Handle to a running sampler thread. Dropping it stops the thread.
#[derive(Debug)]
pub struct SamplerHandle {
    /// Snapshots, one per interval (or per refresh request).
    pub snapshots: Receiver<Snapshot>,
    refresh: Sender<()>,
}

impl SamplerHandle {
    /// Asks for an immediate extra sample.
    pub fn request_refresh(&self) {
        // A closed channel just means the thread already stopped.
        let _ = self.refresh.send(());
    }
}

/// Runs `sampler` on its own thread, emitting a [`Snapshot`] every `interval`.
///
/// # Errors
/// Returns an error if the OS refuses to spawn the thread.
pub fn spawn(interval: Duration, mut sampler: Sampler) -> io::Result<SamplerHandle> {
    let (snap_tx, snapshots) = mpsc::channel();
    let (refresh, refresh_rx) = mpsc::channel::<()>();

    thread::Builder::new()
        .name("sampler".into())
        .spawn(move || {
            loop {
                if snap_tx.send(sampler.sample()).is_err() {
                    break; // UI gone
                }
                match refresh_rx.recv_timeout(interval) {
                    Err(RecvTimeoutError::Disconnected) => break,
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                }
            }
        })?;

    Ok(SamplerHandle { snapshots, refresh })
}
