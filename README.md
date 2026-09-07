# thread

Thinking Log TUI — dated intellectual notes on topics over time. Not a diary.

Stack: Rust + [Ratatui](https://ratatui.rs) + Crossterm. MIT licensed.

## Rust version

Pinned to **1.88** via [`rust-toolchain.toml`](rust-toolchain.toml) (`rustc 1.88.0`).

```bash
rustc --version   # 1.88.0
```

## Run

```bash
cargo run
```

The app enters the alternate screen, shows **thread**, and restores the terminal on `q` (also on panic / drop).

First run creates the notes directory if it is missing (`~/Documents/thread`). `$THREAD_HOME` overrides that path (and the future config directory). `config.toml` is not read yet.

## Note path layout

```
{notes_dir}/{topic}/{yyyy-mm-dd}-{slug}.md
```

YAML front matter:

```yaml
---
topic: rust
title: Ownership notes
status: scratch   # or keep
created: 2026-09-07T12:00:00+00:00
updated: 2026-09-07T12:00:00+00:00
parent:           # optional
---

Note body
```

A file with no `---` fence is still a note: the whole file is the body, topic is the parent directory, and title is the filename.

## Tests

```bash
cargo test
```

## Status

- **M0** — crate boots (alt screen, `q` quits, terminal restored)
- **M1** — filesystem store (`list_topics`, `list_notes`, `load`, `save`, `search`, `latest`)
- Later — two-pane UI, editor, threads / pull-quotes, timer
