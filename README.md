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

The app enters the alternate screen and restores the terminal on `q` (also on panic / drop).

First run creates the notes directory if it is missing (`~/Documents/thread`). `$THREAD_HOME` overrides that path (and the future config directory). `config.toml` is not read yet.

## Keys (M3)

| Key | Mode | Action |
| --- | --- | --- |
| `j` / `k` | Normal | Move topic selection (clamped; most recently updated first) |
| `Enter` | Normal | Reload the selected topic’s **latest** note |
| `i` | Normal | Insert into the open note at the current cursor |
| `a` | Normal | Insert at the end of the open note’s body |
| `n` | Normal | New note: `topic:` (prefill current) then `title:` |
| `Ctrl+s` | Normal / Insert | Save without quitting |
| `Esc` | Insert / Title | Return to Normal (Insert also saves if dirty; Title cancels) |
| `q` | Normal | Save if needed and quit (no confirm) |

Insert editing: type, Backspace, Enter (newline), arrows, Home/End. UTF-8 is char-safe (no byte-offset panics).

Left pane (~28%) lists unique topics. Right pane (~72%) is the **current note** (editable in Insert). The top bar shows `thread` plus the current topic, and when a note is open its date, status, and whitespace word count (updates as you type). The bottom bar hints follow the mode (`Normal` / `Insert` / `Title`).

Dirty notes autosave about every 30 seconds and when leaving Insert.

Not bound yet: `t` thread-detail, `w` scratch/keep, `p` pull-quote, `/` search, `s` timer.

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

New notes start with:

```
What am I trying to decide?

```

## Tests

```bash
cargo test
```

## Status

- **M0** — crate boots (alt screen, `q` quits, terminal restored)
- **M1** — filesystem store (`list_topics`, `list_notes`, `load`, `save`, `search`, `latest`)
- **M2** — two-pane Normal mode (topics + latest note)
- **M3** — Insert / Title editor, save / autosave, new note, word count
- Later — threads / pull-quotes, timer
