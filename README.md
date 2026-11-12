# sloppytoppy

A red-themed [ratatui](https://ratatui.rs) dashboard for your running **Claude Code** and **Codex** processes: live CPU/memory/token graphs, per-session token usage, and safe process termination.

## Install

```sh
cargo install sloppytoppy                                       # once published to crates.io
cargo install --git https://github.com/galenoshea/sloppytoppy   # from source
```

## Usage

```sh
sloppytoppy                 # interactive TUI
sloppytoppy --once          # print one plain-text snapshot (scriptable)
sloppytoppy --interval 500  # sampling interval in ms (200–60000)
sloppytoppy --no-usage      # don't read session logs
```

| Key | Action |
| --- | --- |
| `↑` `↓` / `j` | select |
| `g` / `G` | first / last |
| `Tab` / `←` `→` | filter All / Claude / Codex |
| `k` | terminate (SIGTERM), with confirmation |
| `K` | force kill (SIGKILL), with confirmation |
| `r` | refresh now |
| `q` / `Esc` | quit |

## Token usage

Usage is read from the session logs the CLIs already write — nothing is sent anywhere:

| Tool | Source | Env override |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/<cwd>/<session>.jsonl` | `CLAUDE_CONFIG_DIR` |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | `CODEX_HOME` |

Tokens are normalised into disjoint buckets (input, output, cache read, cache write) plus context-window fill. A process is matched to its session by working directory (newest log wins), so two instances in the *same* directory share one session's numbers. Streamed Claude responses are de-duplicated by message id. Logs are re-parsed only when their size or mtime changes.

## Safety

- Only processes whose executable (or interpreter script) file stem is exactly `claude` or `codex` are listed.
- Before any signal, the PID is re-verified against fresh process state (still claude/codex, owned by you) to close PID-reuse races; a confirmation dialog is always shown.
- `#![forbid(unsafe_code)]`, no network access, read-only access to logs.

## Layout

```
src/
  process/   discovery, background sampler thread, guarded kill
  usage/     UsageProvider trait + Claude and Codex log parsers
  app.rs     state + update logic (no rendering)   input.rs  key bindings
  ui/        stateless ratatui rendering, one module per panel
tests/       provider tests against fixture trees
```

MSRV: Rust 1.88. Licensed MIT.
