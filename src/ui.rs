//! Two-pane Normal-mode view: topics on the left, latest note on the right.

use crate::app::App;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

const NORMAL_HINTS: &str = "j/k select  enter open latest  q quit";

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

    let (title, body_text) = match app.opened() {
        Some(note) => (note.title.as_str(), note.body.as_str()),
        None => ("note", ""),
    };
    frame.render_widget(
        Paragraph::new(body_text)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(title)),
        right,
    );

    frame.render_widget(Paragraph::new(NORMAL_HINTS), bottom);
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
        assert!(rendered.contains("scratch"));
        assert!(rendered.contains("2026-09-07"));
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
}
