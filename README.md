# sloppytoppy

A small [ratatui](https://ratatui.rs) TUI that lists every running `claude` and `codex` process and lets you terminate them.

## Install

```sh
cargo install sloppytoppy          # once published to crates.io
cargo install --git https://github.com/galenoshea/sloppytoppy   # from source
```

Then run `sloppytoppy`.

## Keys

| Key | Action |
| --- | --- |
| `↑` / `↓` (`j`) | select |
| `k` | terminate (SIGTERM), asks to confirm |
| `K` | force kill (SIGKILL), asks to confirm |
| `r` | refresh |
| `q` / `Esc` | quit |

## Safety

- Only processes whose executable (or interpreter script) basename is exactly `claude` or `codex` are listed.
- Kills require confirmation, re-verify the PID still matches, and refuse processes not owned by the current user.
- No `unsafe`, no network access.

## Status

Pre-release; security review pending before publishing to crates.io.
