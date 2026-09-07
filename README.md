# thread

Thinking Log TUI — dated intellectual notes on topics over time. Not a diary.

Stack: Rust + [Ratatui](https://ratatui.rs) + Crossterm. MIT licensed.

## Rust version

Pinned to **1.88** via [`rust-toolchain.toml`](rust-toolchain.toml) (`rustc 1.88.0`).

```bash
rustc --version   # 1.88.0
```

## Install / run

```bash
cargo install --path .
thread
```

Or without installing:

```bash
cargo run
```

The app enters the alternate screen and restores the terminal on `q`, panic, and drop (leave alt screen, disable raw mode).

## Notes path

- Default: `~/Documents/thread`
- `$THREAD_HOME` overrides the notes directory **and** the config directory
- Without `$THREAD_HOME`, config is `~/.config/thread`
- First run creates the notes directory if it is missing
- If `config.toml` already exists, `show_scratch_in_thread` (default **true**) and `session_minutes` (default **20**) are read. The file is never created.

## Keys (M0–M5)

| Key | Mode | Action |
| --- | --- | --- |
| `j` / `k` | Normal | Move selection (topics, or notes in thread view). Clamped, no wrap. |
| `Enter` | Normal | Topics: open that topic’s **latest** note. Thread: open the highlighted note. |
| `t` | Normal | Toggle left pane: all topics ↔ notes in the current topic (oldest at top, newest at bottom) |
| `w` | Normal | Toggle current note `scratch` ↔ `keep` and save immediately |
| `p` | Normal | Append a quoted excerpt (last ~20 lines) from the topic’s last **keep**, or latest if none; enter Insert |
| `/` | Normal | Search mode: incremental case-insensitive substring on titles and bodies |
| `Enter` | Search | Open the selected hit |
| `s` | Normal | Start / stop session countdown (`session_minutes`, default 20). Shown in the top bar; at `0:00` the top bar flashes. Editing stays unlocked. |
| `Esc` | Search / Insert / Title | Return to Normal (Insert also saves if dirty; Title cancels; Search cancels) |
| `i` | Normal | Insert into the open note at the current cursor |
| `a` | Normal | Insert at the end of the open note’s body |
| `n` | Normal | New note: `topic:` (prefill current) then `title:` |
| `Ctrl+s` | Normal / Insert / Search | Save without quitting |
| `q` | Normal | Save if needed and quit (no confirm) |

Insert editing: type, Backspace, Enter (newline), arrows, Home/End. UTF-8 is char-safe (no byte-offset panics).

In Search, type to filter (including `j`/`k`); Up/Down pick among hits. Empty query lists all notes.

Left pane (~28%) is **topics** or **notes** (or search hits). Right pane (~72%) is the **current note** (editable in Insert). The top bar shows `thread` plus the current topic, and when a note is open its date, **status**, whitespace word count, session countdown (if running), and `*` when the open note has unsaved edits. The bottom bar hints follow the mode.

Dirty notes autosave about every 30 seconds and when leaving Insert.

Some host terminals intercept `Ctrl+s` (XON/XOFF or a window-title binding). The TUI still saves via `q` and via in-app `^s` when that key reaches the app.

## Note path layout

```
{notes_dir}/{topic}/{yyyy-mm-dd}-{slug}.md
```

Example:

```
~/Documents/thread/rust/2026-09-07-ownership-notes.md
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
- **M4** — thread notes pane, scratch/keep, pull-quote, search
- **M5** — session timer, dirty indicator, resize, panic/teardown, README
