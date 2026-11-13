//! End-to-end provider tests against on-disk fixture trees.

use std::fs;
use std::path::Path;

use sloppytoppy::usage::{ClaudeProvider, CodexProvider, UsageProvider};
use tempfile::tempdir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn claude_resolves_newest_session_for_cwd() {
    let home = tempdir().unwrap();
    let dir = home.path().join("projects/-work-my-app");
    write(
        &dir.join("session.jsonl"),
        r#"{"sessionId":"s","message":{"id":"a","model":"claude-sonnet-5-5","usage":{"input_tokens":3,"output_tokens":4,"cache_read_input_tokens":5,"cache_creation_input_tokens":6}}}
"#,
    );

    let mut provider = ClaudeProvider::new(home.path());
    let usage = provider.usage_for(Path::new("/work/other")).unwrap();
    assert!(usage.is_none(), "different cwd must not match");

    // Claude's encoding is lossy (`.` and `/` both become `-`), so these collide
    // by design; we match what Claude itself writes.
    let usage = provider
        .usage_for(Path::new("/work/my/app"))
        .unwrap()
        .unwrap();
    assert_eq!(usage.tokens.total(), 18);
    assert_eq!(usage.model.as_deref(), Some("claude-sonnet-5-5"));
}

#[test]
fn claude_reparses_when_log_grows() {
    let home = tempdir().unwrap();
    let log = home.path().join("projects/-w/s.jsonl");
    let line = |id: &str| {
        format!(
            "{{\"message\":{{\"id\":\"{id}\",\"usage\":{{\"input_tokens\":10,\"output_tokens\":0}}}}}}\n"
        )
    };
    write(&log, &line("a"));
    let mut provider = ClaudeProvider::new(home.path());
    let cwd = Path::new("/w");
    assert_eq!(provider.usage_for(cwd).unwrap().unwrap().tokens.total(), 10);

    fs::write(&log, line("a") + &line("b")).unwrap();
    assert_eq!(provider.usage_for(cwd).unwrap().unwrap().tokens.total(), 20);
}

#[test]
fn codex_matches_session_by_recorded_cwd() {
    let home = tempdir().unwrap();
    let day = home.path().join("sessions/2026/11/12");
    let rollout = |id: &str, cwd: &str, total: u64| {
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"cwd\":\"{cwd}\"}}}}\n\
             {{\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":{total},\"output_tokens\":0}},\"last_token_usage\":{{\"input_tokens\":1}}}}}}}}\n"
        )
    };
    write(
        &day.join("rollout-a.jsonl"),
        &rollout("a", "/proj/one", 100),
    );
    write(
        &day.join("rollout-b.jsonl"),
        &rollout("b", "/proj/two", 200),
    );

    let mut provider = CodexProvider::new(home.path());
    let one = provider.usage_for(Path::new("/proj/one")).unwrap().unwrap();
    let two = provider.usage_for(Path::new("/proj/two")).unwrap().unwrap();
    assert_eq!(
        (one.session_id.as_deref(), one.tokens.total()),
        (Some("a"), 100)
    );
    assert_eq!(
        (two.session_id.as_deref(), two.tokens.total()),
        (Some("b"), 200)
    );
    assert!(
        provider
            .usage_for(Path::new("/proj/none"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn missing_directories_are_not_errors() {
    let home = tempdir().unwrap();
    assert!(
        ClaudeProvider::new(home.path())
            .usage_for(Path::new("/x"))
            .unwrap()
            .is_none()
    );
    assert!(
        CodexProvider::new(home.path())
            .usage_for(Path::new("/x"))
            .unwrap()
            .is_none()
    );
}

/// A symlink named `*.jsonl` (e.g. to `/dev/zero`) must never be opened.
#[cfg(unix)]
#[test]
fn symlinked_logs_are_ignored() {
    use std::os::unix::fs::symlink;

    let home = tempdir().unwrap();
    let dir = home.path().join("projects/-w");
    fs::create_dir_all(&dir).unwrap();
    symlink("/dev/zero", dir.join("evil.jsonl")).unwrap();
    let mut claude = ClaudeProvider::new(home.path());
    assert!(claude.usage_for(Path::new("/w")).unwrap().is_none());

    // Same for a symlinked day directory in the Codex tree.
    let target = home.path().join("elsewhere");
    fs::create_dir_all(&target).unwrap();
    write(
        &target.join("rollout-x.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"x\",\"cwd\":\"/w\"}}\n",
    );
    let days = home.path().join("sessions/2026/11");
    fs::create_dir_all(&days).unwrap();
    symlink(&target, days.join("12")).unwrap();
    let mut codex = CodexProvider::new(home.path());
    assert!(codex.usage_for(Path::new("/w")).unwrap().is_none());
}
