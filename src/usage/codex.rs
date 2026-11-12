//! Codex session logs: `<home>/sessions/YYYY/MM/DD/rollout-*.jsonl`.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::Value;

use super::cache::FileCache;
use super::{SessionUsage, TokenUsage, UsageError, UsageProvider};

/// How many of the most recent day-directories to consider.
const RECENT_DAYS: usize = 14;

/// Reads usage from Codex rollout logs.
#[derive(Debug)]
pub struct CodexProvider {
    sessions_dir: PathBuf,
    cache: FileCache,
    /// A rollout's working directory never changes, so cache it forever.
    cwd_by_log: HashMap<PathBuf, Option<PathBuf>>,
}

impl CodexProvider {
    /// Creates a provider rooted at a Codex home directory (usually `~/.codex`).
    #[must_use]
    pub fn new(codex_home: impl AsRef<Path>) -> Self {
        Self {
            sessions_dir: codex_home.as_ref().join("sessions"),
            cache: FileCache::default(),
            cwd_by_log: HashMap::new(),
        }
    }

    /// Locates the home via `CODEX_HOME`, falling back to `~/.codex`.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))?;
        Some(Self::new(dir))
    }

    /// Newest recent rollout whose recorded cwd equals `cwd`.
    fn find_log(&mut self, cwd: &Path) -> io::Result<Option<PathBuf>> {
        let mut logs = recent_logs(&self.sessions_dir)?;
        logs.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, path) in logs {
            let recorded = self
                .cwd_by_log
                .entry(path.clone())
                .or_insert_with(|| read_session_cwd(&path));
            if recorded.as_deref() == Some(cwd) {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }
}

impl UsageProvider for CodexProvider {
    fn usage_for(&mut self, cwd: &Path) -> Result<Option<SessionUsage>, UsageError> {
        let Some(log) = self
            .find_log(cwd)
            .map_err(|e| UsageError::io(&self.sessions_dir, e))?
        else {
            return Ok(None);
        };
        self.cache.get_or_parse(&log, parse).map(Some)
    }
}

/// Subdirectories of `dir`, newest (lexicographically greatest) first.
fn subdirs_desc(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    v.sort_unstable_by(|a, b| b.cmp(a));
    Ok(v)
}

/// `(mtime, path)` of every rollout in the newest [`RECENT_DAYS`] day-directories.
fn recent_logs(root: &Path) -> io::Result<Vec<(SystemTime, PathBuf)>> {
    let mut days = Vec::new();
    'outer: for year in subdirs_desc(root)? {
        for month in subdirs_desc(&year)? {
            for day in subdirs_desc(&month)? {
                days.push(day);
                if days.len() == RECENT_DAYS {
                    break 'outer;
                }
            }
        }
    }
    let mut logs = Vec::new();
    for day in days {
        for entry in fs::read_dir(&day)?.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                logs.push((modified, path));
            }
        }
    }
    Ok(logs)
}

/// Reads the `session_meta` line (always first) and returns its `cwd`.
fn read_session_cwd(path: &Path) -> Option<PathBuf> {
    let mut first = String::new();
    BufReader::new(File::open(path).ok()?)
        .read_line(&mut first)
        .ok()?;
    let envelope: Envelope = serde_json::from_str(&first).ok()?;
    (envelope.kind == "session_meta")
        .then(|| envelope.payload?.get("cwd")?.as_str().map(PathBuf::from))
        .flatten()
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    payload: Option<Value>,
}

/// Field names mirror the wire format.
#[derive(Deserialize, Default)]
#[allow(clippy::struct_field_names)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    cache_write_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    reasoning_output_tokens: u64,
}

impl From<RawUsage> for TokenUsage {
    /// Codex counts cached tokens as a subset of `input_tokens`; split them out
    /// so the buckets are disjoint like Claude's.
    fn from(raw: RawUsage) -> Self {
        Self {
            input: raw.input_tokens.saturating_sub(raw.cached_input_tokens),
            output: raw.output_tokens,
            cache_read: raw.cached_input_tokens,
            cache_write: raw.cache_write_input_tokens,
            reasoning: raw.reasoning_output_tokens,
        }
    }
}

#[derive(Deserialize)]
struct Info {
    #[serde(default)]
    total_token_usage: RawUsage,
    #[serde(default)]
    last_token_usage: RawUsage,
    model_context_window: Option<u64>,
}

/// Parses a rollout. `token_count` events carry *cumulative* totals, so the
/// last non-empty one is the session total.
pub(crate) fn parse(reader: impl BufRead) -> io::Result<SessionUsage> {
    let mut out = SessionUsage::default();
    let mut turns = 0u32;

    for line in reader.lines() {
        let line = line?;
        if !(line.contains("token_count")
            || line.contains("turn_context")
            || line.contains("session_meta")
            || line.contains("task_started"))
        {
            continue;
        }
        let Ok(Envelope {
            kind,
            payload: Some(payload),
        }) = serde_json::from_str(&line)
        else {
            continue;
        };
        match kind.as_str() {
            "session_meta" => {
                out.session_id = payload.get("id").and_then(Value::as_str).map(str::to_owned);
                out.context_window = payload.get("context_window").and_then(Value::as_u64);
            }
            "turn_context" => {
                if let Some(model) = payload.get("model").and_then(Value::as_str) {
                    out.model = Some(model.to_owned());
                }
            }
            "event_msg" => match payload.get("type").and_then(Value::as_str) {
                Some("task_started") => turns += 1,
                Some("token_count") => {
                    let Some(info) = payload.get("info").filter(|v| !v.is_null()) else {
                        continue;
                    };
                    let Ok(info) = serde_json::from_value::<Info>(info.clone()) else {
                        continue;
                    };
                    out.tokens = info.total_token_usage.into();
                    let last = TokenUsage::from(info.last_token_usage);
                    out.context_tokens = last.input + last.cache_read + last.cache_write;
                    out.context_window = info.model_context_window.or(out.context_window);
                }
                _ => {}
            },
            _ => {}
        }
    }
    out.turns = turns;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"{"type":"session_meta","payload":{"id":"abc","cwd":"/work/app"}}
{"type":"turn_context","payload":{"model":"gpt-5-codex"}}
{"type":"event_msg","payload":{"type":"task_started"}}
{"type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{}}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":60,"output_tokens":30,"reasoning_output_tokens":10},"last_token_usage":{"input_tokens":100,"cached_input_tokens":60,"output_tokens":30},"model_context_window":272000}}}
{"type":"event_msg","payload":{"type":"task_started"}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":250,"cached_input_tokens":200,"output_tokens":70,"reasoning_output_tokens":25},"last_token_usage":{"input_tokens":150,"cached_input_tokens":140,"output_tokens":40},"model_context_window":272000}}}
"#;

    #[test]
    fn takes_last_cumulative_total() {
        let u = parse(LOG.as_bytes()).unwrap();
        assert_eq!(u.session_id.as_deref(), Some("abc"));
        assert_eq!(u.model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(u.turns, 2);
        assert_eq!(u.tokens.input, 50);
        assert_eq!(u.tokens.cache_read, 200);
        assert_eq!(u.tokens.output, 70);
        assert_eq!(u.tokens.reasoning, 25);
        assert_eq!(u.tokens.total(), 320);
        assert_eq!(u.context_tokens, 150);
        assert_eq!(u.context_window, Some(272_000));
    }
}
