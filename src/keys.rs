//! Keyboard mapping. Mode-aware: Normal, Insert, Title, Search.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Elm-style message produced from a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Quit,
    SelectDown,
    SelectUp,
    OpenSelected,
    EnterInsert,
    EnterInsertAppend,
    StartNewNote,
    ToggleThread,
    ToggleStatus,
    PullQuote,
    StartSearch,
    ToggleSession,
    Save,
    Escape,
    Submit,
    InsertChar(char),
    Backspace,
    Newline,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Home,
    End,
}

/// Input mode for key dispatch (Title / Search prompt state lives on `App`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Title,
    Search,
}

pub fn message_from_key(key: KeyEvent, mode: InputMode) -> Option<Message> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('s' | 'S') => Some(Message::Save),
            _ => None,
        };
    }
    match mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('q' | 'Q') => Some(Message::Quit),
            KeyCode::Char('j') => Some(Message::SelectDown),
            KeyCode::Char('k') => Some(Message::SelectUp),
            KeyCode::Enter => Some(Message::OpenSelected),
            KeyCode::Char('i') => Some(Message::EnterInsert),
            KeyCode::Char('a') => Some(Message::EnterInsertAppend),
            KeyCode::Char('n') => Some(Message::StartNewNote),
            KeyCode::Char('t') => Some(Message::ToggleThread),
            KeyCode::Char('w') => Some(Message::ToggleStatus),
            KeyCode::Char('p') => Some(Message::PullQuote),
            KeyCode::Char('/') => Some(Message::StartSearch),
            KeyCode::Char('s') => Some(Message::ToggleSession),
            KeyCode::Esc => Some(Message::Escape),
            _ => None,
        },
        InputMode::Insert => match key.code {
            KeyCode::Esc => Some(Message::Escape),
            KeyCode::Char(c) => Some(Message::InsertChar(c)),
            KeyCode::Backspace => Some(Message::Backspace),
            KeyCode::Enter => Some(Message::Newline),
            KeyCode::Left => Some(Message::MoveLeft),
            KeyCode::Right => Some(Message::MoveRight),
            KeyCode::Up => Some(Message::MoveUp),
            KeyCode::Down => Some(Message::MoveDown),
            KeyCode::Home => Some(Message::Home),
            KeyCode::End => Some(Message::End),
            _ => None,
        },
        InputMode::Title => match key.code {
            KeyCode::Esc => Some(Message::Escape),
            KeyCode::Enter => Some(Message::Submit),
            KeyCode::Char(c) => Some(Message::InsertChar(c)),
            KeyCode::Backspace => Some(Message::Backspace),
            KeyCode::Left => Some(Message::MoveLeft),
            KeyCode::Right => Some(Message::MoveRight),
            KeyCode::Home => Some(Message::Home),
            KeyCode::End => Some(Message::End),
            _ => None,
        },
        InputMode::Search => match key.code {
            KeyCode::Esc => Some(Message::Escape),
            KeyCode::Enter => Some(Message::Submit),
            KeyCode::Char(c) => Some(Message::InsertChar(c)),
            KeyCode::Backspace => Some(Message::Backspace),
            KeyCode::Down => Some(Message::SelectDown),
            KeyCode::Up => Some(Message::SelectUp),
            _ => None,
        },
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

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn q_quits_in_normal_only() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('q')), InputMode::Normal),
            Some(Message::Quit)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('Q')), InputMode::Normal),
            Some(Message::Quit)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('q')), InputMode::Insert),
            Some(Message::InsertChar('q'))
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('q')), InputMode::Search),
            Some(Message::InsertChar('q'))
        );
    }

    #[test]
    fn j_k_move_selection() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('j')), InputMode::Normal),
            Some(Message::SelectDown)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('k')), InputMode::Normal),
            Some(Message::SelectUp)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('j')), InputMode::Insert),
            Some(Message::InsertChar('j'))
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('j')), InputMode::Search),
            Some(Message::InsertChar('j'))
        );
    }

    #[test]
    fn enter_opens_in_normal_and_newlines_in_insert() {
        assert_eq!(
            message_from_key(press(KeyCode::Enter), InputMode::Normal),
            Some(Message::OpenSelected)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Enter), InputMode::Insert),
            Some(Message::Newline)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Enter), InputMode::Title),
            Some(Message::Submit)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Enter), InputMode::Search),
            Some(Message::Submit)
        );
    }

    #[test]
    fn i_a_n_and_esc_from_normal() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('i')), InputMode::Normal),
            Some(Message::EnterInsert)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('a')), InputMode::Normal),
            Some(Message::EnterInsertAppend)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('n')), InputMode::Normal),
            Some(Message::StartNewNote)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Esc), InputMode::Normal),
            Some(Message::Escape)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Esc), InputMode::Insert),
            Some(Message::Escape)
        );
    }

    #[test]
    fn m4_keys_from_normal() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('t')), InputMode::Normal),
            Some(Message::ToggleThread)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('w')), InputMode::Normal),
            Some(Message::ToggleStatus)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('p')), InputMode::Normal),
            Some(Message::PullQuote)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('/')), InputMode::Normal),
            Some(Message::StartSearch)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('t')), InputMode::Insert),
            Some(Message::InsertChar('t'))
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('/')), InputMode::Search),
            Some(Message::InsertChar('/'))
        );
    }

    #[test]
    fn ctrl_s_saves_in_every_mode() {
        for mode in [
            InputMode::Normal,
            InputMode::Insert,
            InputMode::Title,
            InputMode::Search,
        ] {
            assert_eq!(
                message_from_key(ctrl(KeyCode::Char('s')), mode),
                Some(Message::Save)
            );
        }
        assert_eq!(
            message_from_key(press(KeyCode::Char('s')), InputMode::Normal),
            Some(Message::ToggleSession)
        );
    }

    #[test]
    fn key_release_is_ignored() {
        let mut key = press(KeyCode::Char('q'));
        key.kind = KeyEventKind::Release;
        assert_eq!(message_from_key(key, InputMode::Normal), None);
    }

    #[test]
    fn s_toggles_session_in_normal_only() {
        assert_eq!(
            message_from_key(press(KeyCode::Char('s')), InputMode::Normal),
            Some(Message::ToggleSession)
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('s')), InputMode::Insert),
            Some(Message::InsertChar('s'))
        );
        assert_eq!(
            message_from_key(press(KeyCode::Char('s')), InputMode::Search),
            Some(Message::InsertChar('s'))
        );
        for code in [KeyCode::Char('J'), KeyCode::Char('K')] {
            assert_eq!(
                message_from_key(press(code), InputMode::Normal),
                None,
                "expected {code:?} unbound in Normal"
            );
        }
    }
}
