# Decisions

Library and behavior choices for M0–M1 that are not spelled out in the product sketch.

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
- Files with no YAML fence are still notes: whole file is `body`, `topic` is the parent directory name, `title` is the filename with `.md` stripped, `status` defaults to `scratch`, timestamps come from mtime when the file exists.
- `list_notes` / `search` skip `.md` files that have a fence but fail YAML parse; `load` returns that error.
- Writes go through a sibling `*.md.tmp` then `rename`.

## TUI extras

`q` / `Q` quit as specified. `Ctrl-C` also quits so raw mode is not a trap. The M0 UI does not list notes yet.
