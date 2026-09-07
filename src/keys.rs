//! Keyboard mapping. M2 Normal mode: topics + quit.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

/// Elm-style message produced from a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Quit,
    TopicDown,
    TopicUp,
    OpenLatest,
}

pub fn message_from_key(key: KeyEvent) -> Option<Message> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match key.code {
        KeyCode::Char('q' | 'Q') => Some(Message::Quit),
        KeyCode::Char('j') => Some(Message::TopicDown),
        KeyCode::Char('k') => Some(Message::TopicUp),
        KeyCode::Enter => Some(Message::OpenLatest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;
    use crossterm::event::KeyModifiers;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn q_quits() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('q'))),
            Some(Message::Quit)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('Q'))),
            Some(Message::Quit)
        );
    }

    #[test]
    fn j_k_move_topic_selection() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('j'))),
            Some(Message::TopicDown)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('k'))),
            Some(Message::TopicUp)
        );
    }

    #[test]
    fn enter_opens_latest() {
        assert_eq!(
            message_from_key(press(KeyCode::Enter)),
            Some(Message::OpenLatest)
        );
    }

    #[test]
    fn key_release_is_ignored() {
        let mut key = press(KeyCode::Char('q'));
        key.kind = KeyEventKind::Release;
        assert_eq!(message_from_key(key), None);
    }

    #[test]
    fn m3_keys_are_unbound() {
        for code in [
            KeyCode::Char('n'),
            KeyCode::Char('t'),
            KeyCode::Char('w'),
            KeyCode::Char('p'),
            KeyCode::Char('/'),
            KeyCode::Char('s'),
            KeyCode::Char('i'),
            KeyCode::Char('J'),
            KeyCode::Char('K'),
        ] {
            assert_eq!(
                message_from_key(press(code)),
                None,
                "expected {code:?} unbound"
            );
        }
    }
}
