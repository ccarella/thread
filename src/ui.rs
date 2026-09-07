//! M0-thin drawing: brand the alt screen so `cargo run` is visibly alive.

use crate::app::App;
use ratatui::layout::Alignment;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, _app: &App) {
    let widget = Paragraph::new("thread")
        .alignment(Alignment::Center)
        .block(Block::bordered().title("thread").title_bottom("q to quit"));
    frame.render_widget(widget, frame.area());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn draws_thread_title() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let app = App::new(store);
        let backend = TestBackend::new(24, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
        assert!(
            rendered.contains("thread"),
            "expected 'thread' in buffer, got {rendered:?}"
        );
    }
}
