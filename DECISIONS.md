# Decisions

Library and behavior choices that are not spelled out in the product sketch.

## Ratatui 0.30 (not 0.29)

Ratatui **0.30.2** has MSRV **1.88.0**, which matches the requested rustc pin. M0 uses `ratatui::init` / `ratatui::restore` (raw mode + alternate screen + panic hook) and a `TerminalGuard` `Drop` impl that always calls `restore`, so panic and RAII both leave the terminal usable.

Crossterm is a direct dependency at 0.29 (Ratatui 0.30's default backend).

## serde_yaml 0.9

Front matter is parsed with `serde_yaml` 0.9. The crate is unmaintained; timestamps are stored as RFC3339 *strings* in YAML so serde_yaml's YAML 1.1 timestamp type does not fight `DateTime<Utc>`. `created` / `updated` also accept `YYYY-MM-DD` (midnight UTC) on read.

A later swap to `serde_yml` / `serde_norway` should be mechanical.

## Config file is not loaded yet

`Config::load()` returns defaults only. No `config.toml` is created or parsed.

- If `$THREAD_HOME` is set, it is both `notes_dir` and the future config directory.
- Otherwise notes dir is `~/Documents/thread` and config dir is `~/.config/thread`.

Store tests pass an explicit temp directory and never touch the real notes dir.

## Library + thin binary

Modules match the sketch (`app`, `ui`, `keys`, `note`, `store`, `config`, `error`) but live in a `lib.rs` so the store APIs are a real library with unit tests. `main.rs` only calls `thread::run()`.

## Paths, slugs, and listing

- Filename date is the UTC calendar date of `created`.
- Slugs are ASCII kebab-case from the title; empty / non-latin titles become `note`.
- `save` on a note with no `path` allocates `{topic}/{yyyy-mm-dd}-{slug}.md` and appends `-2`, `-3`, … on collision. Once `path` is set, later saves keep that file (title edits do not rename).
- `parent` is an opaque optional string (relative path recommended); no graph walk in M1.
- Topic names cannot be empty, `.` / `..`, hidden (`.*`), or contain path separators.
- Files with no YAML fence are still notes: whole file is `body`, `topic` is the parent directory name, `title` is the filename (as on disk), `status` defaults to `scratch`, timestamps come from mtime when the file exists.
- `list_notes` / `search` skip `.md` files that have a fence but fail YAML parse; `load` returns that error.
- Writes go through a sibling `*.md.tmp` then `rename`.

## TUI (M0)

`q` / `Q` quit. The M0 screen was the word **thread** plus a border; it did not list notes or bind store keys.

## M2 two-pane Normal mode

Elm-style split: `App` is the model, `keys::Message` is the message (renamed from M0 `Command` now that there is real interaction), `App::update` applies it, `ui::render` is the view.

- `list_topics()` is now recency-ordered (latest note `updated` desc, then name). Empty topic directories (no readable notes) sort last. M1 listed names A–Z; the M2 AC wants most recently updated first.
- Left/right split is `Constraint::Percentage(28)` / `Percentage(72)`.
- `j` / `k` move the topic highlight and clamp at the ends (no wrap, no arrow keys). The right pane always follows the selection: empty, or that topic’s `latest` note, read-only. `Enter` reloads `latest` from disk for the current topic.
- Top bar is `thread` plus the selected topic. When a note is open, it also shows the note’s `created` UTC date, status, and whitespace word count (cheap extras; not required to PASS).
- Bottom bar is Normal-mode hints only. Insert/Title modes and `n` / `t` / `w` / `p` / `/` / `s` stay unbound.
- `q` calls `save_if_needed` then quits with no confirm modal. M2 has no editor, so that save is a no-op.

## M3 editor (Insert / Title)

No extra crate. Body editing is a small char-index buffer in `editor.rs` (Unicode scalar cursor, never a byte offset). `tui-textarea` / `tui-textarea-2` would pull Emacs bindings, undo, and wrap that M3 does not need.

- **Modes:** `Normal` (topics), `Insert` (body), `Title` (new-note prompts). `Esc` always returns to Normal. Title `Esc` cancels and does not create a note.
- **`i` / `a`:** only from Normal, and only when a note is already open. Opening a note puts the cursor at the start of the body; `i` inserts there; `a` jumps to the end of the body then inserts. In Insert, `q` is the letter q (quit is Normal-only). `s` without Ctrl is still unbound (timer is M4/M5).
- **Save:** `Ctrl+s` writes if dirty and stays in the current mode. `q` in Normal saves then quits. Leaving Insert (`Esc`) saves if dirty. A dirty buffer also autosaves every 30s from the first unsaved edit (not a keystroke debounce). `updated` is bumped in `App` on save, not in `Store::save` (so M1 fixtures keep their timestamps).
- **New note `n`:** Title mode, two steps on the bottom bar: `topic:` (prefilled with the selected topic) then `title:`. Empty or invalid topic stays on the prompt. Created notes are `scratch` with body `What am I trying to decide?\n\n` and land in Insert with the cursor after that starter. Path allocation is still `Store::save`.
- **Right pane:** the current note is the edit buffer. Insert draws an unwrapped body plus a terminal cursor; Normal still wraps for reading.
- Still unbound: `t` / `w` / `p` / `/` / `s` (M4/M5).
