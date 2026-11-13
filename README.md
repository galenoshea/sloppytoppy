# sloppytoppy

**`top` for AI coding agents.** Watch Claude Code and Codex processes, see what each session is consuming, and stop a runaway agent without leaving the terminal.

`sloppytoppy` combines live process telemetry with the token data already recorded by each agent CLI. It provides:

- live CPU, memory, uptime, and token totals
- per-session model, context-window, cache, and turn details
- filters for Claude Code and Codex processes
- guarded `SIGTERM` and `SIGKILL` actions with confirmation
- a plain-text snapshot mode for scripts and shell pipelines

## Install

### Prebuilt binary

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/galenoshea/sloppytoppy/releases/latest/download/sloppytoppy-installer.sh | sh
```

The installer detects Apple Silicon macOS, Intel macOS, x86-64 Linux, and ARM64 Linux, then installs the matching executable in Cargo's bin directory. It does not require Rust.

### Cargo

```sh
cargo install sloppytoppy --locked
```

Installing through Cargo requires Rust 1.88 or newer.

## Usage

```sh
sloppytoppy                 # open the live dashboard
sloppytoppy --once          # print one plain-text snapshot and exit
sloppytoppy --interval 500  # sample every 500 ms (range: 200–60000)
sloppytoppy --no-usage      # skip session logs and token accounting
```

### Controls

| Key | Action |
| --- | --- |
| `↑` | Move the selection up |
| `↓` or `j` | Move the selection down |
| `g` / `G` | Jump to the first / last process |
| `Tab` or `←` / `→` | Filter All / Claude / Codex |
| `k` | Request termination (`SIGTERM`) |
| `K` | Force kill (`SIGKILL`) |
| `r` | Refresh now |
| `q`, `Esc`, or `Ctrl-C` | Quit |

## Token usage

Usage comes from the local session logs the CLIs already write. `sloppytoppy` does not send this data anywhere or make network requests.

| Agent | Local source | Environment override |
| --- | --- | --- |
| Claude Code | `~/.claude/projects/<cwd>/<session>.jsonl` | `CLAUDE_CONFIG_DIR` |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | `CODEX_HOME` |

Tokens are normalized into non-overlapping input, output, cache-read, and cache-write buckets, plus context-window fill. A process is matched to the newest session log for its working directory, so two agents running in the same directory may show the same session totals. Streamed Claude responses are de-duplicated by message ID, and logs are only re-parsed when their size or modification time changes.

## Safety and privacy

- Process discovery only accepts an executable name—or a supported interpreter script name—whose file stem is exactly `claude` or `codex`.
- Before sending a signal, the PID is checked against fresh process state to confirm that it is still a recognized agent process owned by the current user.
- Every signal requires confirmation. `k` requests a graceful shutdown; `K` is the explicit force-kill path.
- Session logs are read-only, the application has no network client, and unsafe Rust is forbidden.

## Project structure

```text
src/
  process/   process discovery, background sampling, and guarded signals
  usage/     Claude Code and Codex session-log providers
  ui/        stateless terminal rendering, split by panel
  app.rs     application state and update logic
  input.rs   key bindings
  report.rs  plain-text output for --once
tests/       provider tests against temporary fixture trees
```

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Releases are built from version tags. See [`RELEASING.md`](RELEASING.md) for the maintainer workflow.

Licensed under the MIT License.
