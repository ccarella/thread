# Decisions

Library and behavior choices that are not spelled out in the product sketch.

## Ratatui 0.30 (not 0.29)

Ratatui **0.30.2** has MSRV **1.88.0**, which matches the requested rustc pin. M0 uses `ratatui::init` / `ratatui::restore` (raw mode + alternate screen + panic hook) and a `TerminalGuard` `Drop` impl that always calls `restore`, so panic and RAII both leave the terminal usable.

Crossterm is a direct dependency at 0.29 (Ratatui 0.30's default backend).

## serde_yaml 0.9

Front matter is parsed with `serde_yaml` 0.9. The crate is unmaintained; timestamps are stored as RFC3339 *strings* in YAML so serde_yaml's YAML 1.1 timestamp type does not fight `DateTime<Utc>`. `created` / `updated` also accept `YYYY-MM-DD` (midnight UTC) on read.

A later swap to `serde_yml` / `serde_norway` should be mechanical.

## Config file (M4–M5)

`Config::load()` still uses directory defaults from M0:

- If `$THREAD_HOME` is set, it is both `notes_dir` and the config directory.
- Otherwise notes dir is `~/Documents/thread` and config dir is `~/.config/thread`.

If `config.toml` already exists in the config directory, `show_scratch_in_thread` (BUILD_SPEC default **true** when the file or key is missing) and `session_minutes` (BUILD_SPEC default **20**; `0` / unparsable keep the default) are honored. The file is never created. Other keys, including `notes_dir`, are ignored. Parsing is a tiny line scan, not a TOML crate.

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
- **`i` / `a`:** only from Normal, and only when a note is already open. Opening a note puts the cursor at the start of the body; `i` inserts there; `a` jumps to the end of the body then inserts. In Insert, `q` is the letter q (quit is Normal-only).
- **Save:** `Ctrl+s` writes if dirty and stays in the current mode. `q` in Normal saves then quits. Leaving Insert (`Esc`) saves if dirty. A dirty buffer also autosaves every 30s from the first unsaved edit (not a keystroke debounce). `updated` is bumped in `App` on save, not in `Store::save` (so M1 fixtures keep their timestamps).
- **New note `n`:** Title mode, two steps on the bottom bar: `topic:` (prefilled with the selected topic) then `title:`. Empty or invalid topic stays on the prompt. Created notes are `scratch` with body `What am I trying to decide?\n\n` and land in Insert with the cursor after that starter. Path allocation is still `Store::save`.
- **Right pane:** the current note is the edit buffer. Insert draws an unwrapped body plus a terminal cursor; Normal still wraps for reading.
- M3 left `t` / `w` / `p` / `/` / `s` unbound.

## M4 thread, status, pull-quote, search

- **`t`:** left pane toggles `topics` ↔ `notes` for the current topic. Notes are chronological by `created` (oldest at top, newest at bottom). `j`/`k` only move the highlight in notes mode; `Enter` opens that note. In topics mode `j`/`k` still follow the M2 preview (latest) and `Enter` still reloads latest. Hidden scratch notes (`show_scratch_in_thread = false`) are omitted from this list only, not from search or `latest`.
- **`w`:** toggles `scratch`/`keep` on the open note, writes immediately (including any dirty body), bumps `updated`, refreshes the top-bar status. No-op if nothing is open.
- **`p`:** source is the most recently *updated* `keep` in the current note’s topic, else `latest`. Last 20 body lines, prefixed with `>` plus a `> YYYY-MM-DD` date header (`created` of the source). Appended to the open note; leaves Insert and dirty. If the only note is the current one, that note is quoted into itself. No-op with no open note.
- **`/`:** Search mode. Incremental case-insensitive substring via the existing `Store::search` API (titles, bodies, and topic names). `Enter` opens the selected hit (default: most recently updated); `Esc` returns to Normal without changing the open note. Printable characters including `j`/`k` go into the query; **Up/Down** move among hits (so `/` remains a true substring filter). Opening a hit restores the topics pane on that note’s topic.
- `Message` names `SelectDown` / `SelectUp` / `OpenSelected` replace M2’s `TopicDown` / `TopicUp` / `OpenLatest` now that the left pane is not always topics.

## M5 session timer, dirty, resize, teardown

- **`s`:** Normal-only toggle. Starts a countdown from `session_minutes` (default 20) shown in the top bar as `m:ss` (seconds rounded up so a fresh start reads `20:00`). A second `s` clears it. At `0:00` the **top bar** reverses each ~1s poll (`session_flash`); Insert / save / navigation stay enabled. Remaining time is a deadline `Instant`, not a decrementing counter.
- **Poll:** idle loop stays 250ms (autosave). While the session is running, poll is **1s** (BUILD_SPEC) so the countdown and zero-flash redraw without a keypress. Pending keys still wake `poll` immediately. `Event::Resize` is consumed (not treated as a key); the next `draw` autoresizes. Layout stays `Percentage(28)` / `Percentage(72)` with `Min(1)` on the body.
- **Dirty:** top bar `*` while the open note has unsaved edits. Clears on save / autosave / `w` persist. Not a modal.
- **Panic / teardown:** unchanged mechanism from M0 — `ratatui::init` installs a panic hook and `TerminalGuard` `Drop` always calls `ratatui::restore()` (leave alt screen + disable raw mode). `q` drops the guard on the way out. `kill -9` cannot run destructors; SIGTERM is not hooked (no extra crate). Host-terminal `Ctrl+s` (XOFF / title binding) is documented in the README; save still works via `q` / in-app `^s`.
- No §8 extras (no AI, sync, diary, agent dispatch, vim).
