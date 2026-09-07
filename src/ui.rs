//! Two-pane view: topics on the left, editable current note on the right.

use crate::app::{App, TitleStep};
use crate::editor;
use crate::keys::InputMode;
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

const NORMAL_HINTS: &str = "j/k select  enter open latest  i/a insert  n new  ^s save  q quit";
const INSERT_HINTS: &str = "esc normal  ^s save";

pub fn render(frame: &mut Frame, app: &App) {
    let [top, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(28), Constraint::Percentage(72)]).areas(body);

    frame.render_widget(Paragraph::new(top_bar(app)), top);

    let items: Vec<ListItem> = app
        .topics()
        .iter()
        .map(|topic| ListItem::new(topic.as_str()))
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title("topics"))
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    if !app.topics().is_empty() {
        state.select(Some(app.selected()));
    }
    frame.render_stateful_widget(list, left, &mut state);

    render_note(frame, app, right);
    render_bottom(frame, app, bottom);
}

fn render_note(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (title, body_text) = match app.opened() {
        Some(note) => (note.title.as_str(), note.body.as_str()),
        None => ("note", ""),
    };
    let inserting = app.input_mode() == InputMode::Insert && app.opened().is_some();
    let block = Block::bordered().title(title);
    let inner = block.inner(area);

    if inserting {
        let (col, line) = {
            let (line, col, _) = editor::line_col(body_text, app.cursor());
            (col as u16, line as u16)
        };
        let max_y = inner.height.saturating_sub(1);
        let scroll_y = line.saturating_sub(max_y);
        frame.render_widget(
            Paragraph::new(body_text).scroll((scroll_y, 0)).block(block),
            area,
        );
        if inner.width > 0 && inner.height > 0 {
            let x = inner.x + col.min(inner.width.saturating_sub(1));
            let y = inner.y + line.saturating_sub(scroll_y).min(max_y);
            frame.set_cursor_position(Position::new(x, y));
        }
    } else {
        frame.render_widget(
            Paragraph::new(body_text)
                .wrap(Wrap { trim: false })
                .block(block),
            area,
        );
    }
}

fn render_bottom(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    match app.input_mode() {
        InputMode::Normal => {
            frame.render_widget(Paragraph::new(NORMAL_HINTS), area);
        }
        InputMode::Insert => {
            frame.render_widget(Paragraph::new(INSERT_HINTS), area);
        }
        InputMode::Title => {
            let Some(state) = app.title_state() else {
                return;
            };
            let label = match state.step {
                TitleStep::Topic => "topic",
                TitleStep::Title => "title",
            };
            let input = state.input();
            let mut line = format!("{label}: {input}");
            if let Some(error) = &state.error {
                line.push_str("  ");
                line.push_str(error);
            }
            frame.render_widget(Paragraph::new(line), area);
            if area.width > 0 && area.height > 0 {
                let prefix = label.len() + 2; // "topic: " / "title: "
                let x = area.x
                    + (prefix + state.cursor).min(area.width.saturating_sub(1) as usize) as u16;
                frame.set_cursor_position(Position::new(x, area.y));
            }
        }
    }
}

fn top_bar(app: &App) -> String {
    let mut parts = vec!["thread".to_string()];
    if let Some(topic) = app.selected_topic() {
        parts.push(topic.to_string());
    }
    if let Some(note) = app.opened() {
        parts.push(note.created.format("%Y-%m-%d").to_string());
        parts.push(note.status.to_string());
        parts.push(format!("{}w", note.word_count()));
    }
    match app.input_mode() {
        InputMode::Insert => parts.push("insert".into()),
        InputMode::Title => parts.push("title".into()),
        InputMode::Normal => {}
    }
    parts.join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::keys::Message;
    use crate::note::Note;
    use crate::store::Store;
    use chrono::{TimeZone, Utc};
    use ratatui::{backend::TestBackend, Terminal};

    fn render_text(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn seeded_app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut philosophy = Note::new("philosophy", "Forms", "plato");
        philosophy.created = Utc.with_ymd_and_hms(2026, 8, 1, 10, 0, 0).unwrap();
        philosophy.updated = philosophy.created;
        let mut rust = Note::new("rust", "Ownership", "borrow checker thoughts");
        rust.created = Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap();
        rust.updated = rust.created;
        store.save(&mut philosophy).unwrap();
        store.save(&mut rust).unwrap();
        (App::new(store).unwrap(), dir)
    }

    #[test]
    fn draws_thread_title() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(Store::open(dir.path()).unwrap()).unwrap();
        let rendered = render_text(&app, 24, 8);
        assert!(
            rendered.contains("thread"),
            "expected 'thread' in buffer, got {rendered:?}"
        );
    }

    #[test]
    fn two_panes_show_topics_latest_note_and_normal_hints() {
        let (app, _dir) = seeded_app();
        let rendered = render_text(&app, 80, 12);
        assert!(rendered.contains("thread"));
        assert!(rendered.contains("topics"));
        assert!(rendered.contains("rust"));
        assert!(rendered.contains("philosophy"));
        assert!(rendered.contains("Ownership"));
        assert!(rendered.contains("borrow checker thoughts"));
        assert!(rendered.contains("j/k select"));
        assert!(rendered.contains("enter open latest"));
        assert!(rendered.contains("q quit"));
        assert!(rendered.contains("i/a insert"));
        assert!(rendered.contains("n new"));
        assert!(rendered.contains("scratch"));
        assert!(rendered.contains("2026-09-07"));
        assert!(rendered.contains("3w"));
    }

    #[test]
    fn selecting_another_topic_shows_its_latest_note() {
        let (mut app, _dir) = seeded_app();
        app.update(Message::TopicDown).unwrap();
        let rendered = render_text(&app, 80, 12);
        assert!(rendered.contains("Forms"));
        assert!(rendered.contains("plato"));
        assert!(!rendered.contains("borrow checker thoughts"));
    }

    #[test]
    fn empty_store_shows_empty_note_pane_and_hints() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(Store::open(dir.path()).unwrap()).unwrap();
        let rendered = render_text(&app, 80, 12);
        assert!(rendered.contains("thread"));
        assert!(rendered.contains("topics"));
        assert!(rendered.contains("note"));
        assert!(rendered.contains("j/k select"));
        assert!(!rendered.contains("scratch"));
    }

    #[test]
    fn insert_mode_changes_hints_and_word_count_on_edit() {
        let (mut app, _dir) = seeded_app();
        assert!(render_text(&app, 80, 12).contains("3w"));

        app.update(Message::EnterInsertAppend).unwrap();
        app.update(Message::InsertChar(' ')).unwrap();
        app.update(Message::InsertChar('x')).unwrap();
        let rendered = render_text(&app, 80, 12);
        assert!(rendered.contains("esc normal"));
        assert!(rendered.contains("^s save"));
        assert!(rendered.contains("insert"));
        assert!(!rendered.contains("j/k select"));
        assert!(rendered.contains("4w"));
    }

    #[test]
    fn title_mode_shows_topic_prompt() {
        let (mut app, _dir) = seeded_app();
        app.update(Message::StartNewNote).unwrap();
        let rendered = render_text(&app, 80, 12);
        assert!(rendered.contains("topic: rust"));
        assert!(rendered.contains("title"));
    }
}
