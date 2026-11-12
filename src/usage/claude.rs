//! Claude Code session logs: `<config>/projects/<encoded-cwd>/<session>.jsonl`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

use super::cache::FileCache;
use super::{SessionUsage, TokenUsage, UsageError, UsageProvider};

/// Reads usage from Claude Code transcripts.
#[derive(Debug)]
pub struct ClaudeProvider {
    projects_dir: PathBuf,
    cache: FileCache,
}

impl ClaudeProvider {
    /// Creates a provider rooted at a Claude config directory (usually `~/.claude`).
    #[must_use]
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            projects_dir: config_dir.as_ref().join("projects"),
            cache: FileCache::default(),
        }
    }

    /// Locates the config directory via `CLAUDE_CONFIG_DIR`, falling back to `~/.claude`.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".claude")))?;
        Some(Self::new(dir))
    }
}

impl UsageProvider for ClaudeProvider {
    fn usage_for(&mut self, cwd: &Path) -> Result<Option<SessionUsage>, UsageError> {
        let dir = self.projects_dir.join(project_dir_name(cwd));
        let Some(newest) = newest_log(&dir).map_err(|e| UsageError::io(&dir, e))? else {
            return Ok(None);
        };
        self.cache.get_or_parse(&newest, parse).map(Some)
    }
}

/// Claude encodes a working directory by replacing every non-alphanumeric
/// character with `-` (`/Users/me/app.rs` becomes `-Users-me-app-rs`).
fn project_dir_name(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Newest `*.jsonl` file in `dir`, or `None` if the directory is absent/empty.
fn newest_log(dir: &Path) -> io::Result<Option<PathBuf>> {
    let read = match fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
            best = Some((modified, path));
        }
    }
    Ok(best.map(|(_, p)| p))
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<RawUsage>,
}

/// Field names mirror the wire format.
#[derive(Deserialize, Default)]
#[allow(clippy::struct_field_names)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    output_tokens_details: Option<OutputDetails>,
}

#[derive(Deserialize)]
struct OutputDetails {
    #[serde(default)]
    thinking_tokens: u64,
}

impl From<RawUsage> for TokenUsage {
    fn from(raw: RawUsage) -> Self {
        Self {
            input: raw.input_tokens,
            output: raw.output_tokens,
            cache_read: raw.cache_read_input_tokens,
            cache_write: raw.cache_creation_input_tokens,
            reasoning: raw.output_tokens_details.map_or(0, |d| d.thinking_tokens),
        }
    }
}

/// Parses a transcript.
///
/// A streamed response is logged as several lines sharing one `message.id`; the
/// last occurrence carries the final counts, so usage is de-duplicated by id.
pub(crate) fn parse(reader: impl BufRead) -> io::Result<SessionUsage> {
    let mut by_id: HashMap<String, TokenUsage> = HashMap::new();
    let mut anonymous = TokenUsage::default();
    let mut anonymous_turns = 0u32;
    let mut out = SessionUsage::default();

    for line in reader.lines() {
        let line = line?;
        // Cheap pre-filter: most lines (snapshots, attachments) carry no usage.
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<Line>(&line) else {
            continue;
        };
        let Some(message) = parsed.message else {
            continue;
        };
        let Some(raw) = message.usage else { continue };
        if message.model.as_deref() == Some("<synthetic>") {
            continue;
        }

        out.session_id = out.session_id.or(parsed.session_id);
        if message.model.is_some() {
            out.model = message.model;
        }
        let usage = TokenUsage::from(raw);
        out.context_tokens = usage.input + usage.cache_read + usage.cache_write;
        if let Some(id) = message.id {
            by_id.insert(id, usage);
        } else {
            anonymous += usage;
            anonymous_turns += 1;
        }
    }

    out.tokens = by_id.values().sum::<TokenUsage>() + anonymous;
    out.turns = u32::try_from(by_id.len())
        .unwrap_or(u32::MAX)
        .saturating_add(anonymous_turns);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"{"type":"user","sessionId":"s1","message":{"role":"user","content":"hi"}}
{"type":"assistant","sessionId":"s1","message":{"id":"m1","model":"claude-sonnet-5-5","usage":{"input_tokens":2,"output_tokens":5,"cache_read_input_tokens":100,"cache_creation_input_tokens":10}}}
{"type":"assistant","sessionId":"s1","message":{"id":"m1","model":"claude-sonnet-5-5","usage":{"input_tokens":2,"output_tokens":50,"cache_read_input_tokens":100,"cache_creation_input_tokens":10,"output_tokens_details":{"thinking_tokens":20}}}}
{"type":"assistant","sessionId":"s1","message":{"id":"m2","model":"claude-opus-5-5","usage":{"input_tokens":1,"output_tokens":7,"cache_read_input_tokens":200,"cache_creation_input_tokens":0}}}
{"type":"assistant","message":{"id":"m3","model":"<synthetic>","usage":{"input_tokens":9,"output_tokens":9}}}
not json but mentions "usage"
"#;

    #[test]
    fn dedupes_streamed_messages_and_sums() {
        let u = parse(LOG.as_bytes()).unwrap();
        assert_eq!(u.session_id.as_deref(), Some("s1"));
        assert_eq!(u.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(u.turns, 2);
        assert_eq!(u.tokens.input, 3);
        assert_eq!(u.tokens.output, 57);
        assert_eq!(u.tokens.cache_read, 300);
        assert_eq!(u.tokens.cache_write, 10);
        assert_eq!(u.tokens.reasoning, 20);
        assert_eq!(u.context_tokens, 201);
    }

    #[test]
    fn encodes_project_dir() {
        assert_eq!(
            project_dir_name(Path::new("/Users/me/app.rs")),
            "-Users-me-app-rs"
        );
    }
}
