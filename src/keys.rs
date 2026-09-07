//! Keyboard mapping. M0 only needs quit.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Quit,
}

pub fn command_from_key(key: KeyEvent) -> Option<Command> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    match key.code {
        KeyCode::Char('q' | 'Q') => Some(Command::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Command::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

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
            command_from_key(press(KeyCode::Char('q'))),
            Some(Command::Quit)
        );
        assert_eq!(
            command_from_key(press(KeyCode::Char('Q'))),
            Some(Command::Quit)
        );
    }

    #[test]
    fn key_release_is_ignored() {
        let mut key = press(KeyCode::Char('q'));
        key.kind = KeyEventKind::Release;
        assert_eq!(command_from_key(key), None);
    }

    #[test]
    fn other_keys_do_nothing() {
        assert_eq!(command_from_key(press(KeyCode::Char('x'))), None);
    }
}
