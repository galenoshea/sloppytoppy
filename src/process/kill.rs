//! Guarded signalling: never touch a PID we haven't re-verified.

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind};
use thiserror::Error;

use super::Tool;

/// How hard to stop a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kill {
    /// `SIGTERM`: ask nicely.
    Terminate,
    /// `SIGKILL`: no cleanup.
    Force,
}

impl Kill {
    const fn signal(self) -> Signal {
        match self {
            Self::Terminate => Signal::Term,
            Self::Force => Signal::Kill,
        }
    }
}

/// Why a signal was not delivered.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KillError {
    /// The PID vanished or was reused by something that is not claude/codex.
    #[error("process {0} is gone or is no longer a claude/codex process")]
    Gone(Pid),
    /// The process belongs to another user.
    #[error("refusing to signal {0}: not owned by the current user")]
    NotOwner(Pid),
    /// The OS rejected the signal.
    #[error("the OS refused to signal {0}")]
    Refused(Pid),
}

/// Signals `pid` after re-checking, against fresh process state, that it is
/// still a claude/codex process owned by the current user. This closes the
/// window in which a PID could have been recycled since the last sample.
///
/// # Errors
/// See [`KillError`].
pub fn signal(pid: Pid, how: Kill) -> Result<Tool, KillError> {
    let me = sysinfo::get_current_pid().map_err(|_| KillError::NotOwner(pid))?;
    let mut sys = System::new();
    let kind = ProcessRefreshKind::nothing()
        .with_cmd(UpdateKind::Always)
        .with_user(UpdateKind::Always);
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid, me]), true, kind);

    let process = sys.process(pid).ok_or(KillError::Gone(pid))?;
    let argv: Vec<String> = process
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let tool =
        Tool::classify(&process.name().to_string_lossy(), &argv).ok_or(KillError::Gone(pid))?;

    let my_uid = sys.process(me).and_then(|p| p.user_id());
    if my_uid.is_none() || process.user_id() != my_uid {
        return Err(KillError::NotOwner(pid));
    }

    match process.kill_with(how.signal()) {
        Some(true) => Ok(tool),
        _ => Err(KillError::Refused(pid)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_nonexistent_pid() {
        // PID well above any realistic pid_max.
        let pid = Pid::from_u32(u32::MAX - 1);
        assert_eq!(signal(pid, Kill::Terminate), Err(KillError::Gone(pid)));
    }

    #[test]
    fn refuses_non_agent_process() {
        // The test binary itself is neither claude nor codex.
        let me = sysinfo::get_current_pid().unwrap();
        assert_eq!(signal(me, Kill::Terminate), Err(KillError::Gone(me)));
    }
}
