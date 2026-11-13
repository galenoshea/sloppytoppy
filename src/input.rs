//! Key bindings: pure mapping from key events to [`Action`]s.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::process::Kill;

/// Something the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Leave the application.
    Quit,
    /// Move selection down.
    Next,
    /// Move selection up.
    Prev,
    /// Jump to the first row.
    First,
    /// Jump to the last row.
    Last,
    /// Next tool filter.
    NextFilter,
    /// Previous tool filter.
    PrevFilter,
    /// Resample immediately.
    Refresh,
    /// Open the kill confirmation for the selected process.
    AskKill(Kill),
    /// Confirm the open dialog.
    Confirm,
    /// Dismiss the open dialog.
    Cancel,
}

/// Maps a key press to an action. While a confirmation dialog is open only
/// confirm/cancel keys are live, so a stray `q` can't quit mid-prompt.
#[must_use]
pub fn map_key(key: KeyEvent, dialog_open: bool) -> Option<Action> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }
    if dialog_open {
        return Some(match key.code {
            KeyCode::Char('y' | 'Y') => Action::Confirm,
            _ => Action::Cancel,
        });
    }
    Some(match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Down | KeyCode::Char('j') => Action::Next,
        KeyCode::Up => Action::Prev,
        KeyCode::Home | KeyCode::Char('g') => Action::First,
        KeyCode::End | KeyCode::Char('G') => Action::Last,
        KeyCode::Tab | KeyCode::Right => Action::NextFilter,
        KeyCode::BackTab | KeyCode::Left => Action::PrevFilter,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('k') => Action::AskKill(Kill::Terminate),
        KeyCode::Char('K') => Action::AskKill(Kill::Force),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn normal_mode() {
        assert_eq!(
            map_key(press(KeyCode::Char('q')), false),
            Some(Action::Quit)
        );
        assert_eq!(
            map_key(press(KeyCode::Char('K')), false),
            Some(Action::AskKill(Kill::Force))
        );
        assert_eq!(map_key(press(KeyCode::Char('z')), false), None);
    }

    #[test]
    fn dialog_swallows_other_keys() {
        assert_eq!(
            map_key(press(KeyCode::Char('y')), true),
            Some(Action::Confirm)
        );
        assert_eq!(
            map_key(press(KeyCode::Char('q')), true),
            Some(Action::Cancel)
        );
        // Enter must never confirm a kill: only an explicit `y` does.
        assert_eq!(map_key(press(KeyCode::Enter), true), Some(Action::Cancel));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key(ctrl_c, true), Some(Action::Quit));
    }
}
