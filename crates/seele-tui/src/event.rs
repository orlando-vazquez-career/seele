//! Key → Action mapping. Pure function `handle_key` so the keymap is
//! trivially unit-testable in isolation from the event loop.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Pane};

/// High-level intent decoded from a `KeyEvent`. The event loop
/// dispatches these against the service so the keymap stays free of
/// side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    SwitchPane(Pane),
    Up,
    Down,
    Top,
    Bottom,
    Refresh,
    SearchEditAppend(char),
    SearchEditBackspace,
    SearchSubmit,
    SearchClear,
    OpenDetail,
    Back,
}

pub fn handle_key(state: &AppState, key: KeyEvent) -> Option<Action> {
    // Ctrl-C always wins.
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Action::Quit);
    }
    // Search pane consumes printable input first so digits / '/' /
    // 'q' typed into a query do not trigger global pane hotkeys.
    if state.current == Pane::Search {
        if let Some(a) = search_key(key) {
            return Some(a);
        }
        // Fall through for Esc/Enter handled above + other globals.
    }
    if let Some(a) = global_key(key) {
        return Some(a);
    }
    match state.current {
        Pane::Search => None,
        Pane::Detail => detail_key(key),
        Pane::Browse => list_key(key),
        Pane::Home | Pane::Stats => simple_key(key),
    }
}

fn global_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('1') => Some(Action::SwitchPane(Pane::Home)),
        KeyCode::Char('2') => Some(Action::SwitchPane(Pane::Browse)),
        // '3' is reserved for Search pane switch only when not actively
        // typing inside Search (handled by `search_key` returning None
        // for chars, then falling through here would loop). Instead,
        // we route Search input through `search_key` BEFORE this is
        // reached (see `handle_key`), so '3' here is always a pane
        // hotkey.
        KeyCode::Char('3') => Some(Action::SwitchPane(Pane::Search)),
        KeyCode::Char('5') => Some(Action::SwitchPane(Pane::Stats)),
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('r') if key.modifiers.is_empty() => Some(Action::Refresh),
        _ => None,
    }
}

fn list_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Char('g') => Some(Action::Top),
        KeyCode::Char('G') => Some(Action::Bottom),
        KeyCode::Enter => Some(Action::OpenDetail),
        KeyCode::Char('/') => Some(Action::SwitchPane(Pane::Search)),
        _ => None,
    }
}

fn search_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Enter => Some(Action::SearchSubmit),
        KeyCode::Esc => Some(Action::SearchClear),
        KeyCode::Backspace => Some(Action::SearchEditBackspace),
        KeyCode::Up => Some(Action::Up),
        KeyCode::Down => Some(Action::Down),
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Typing inside Search must NOT trigger pane hotkeys.
            // We get here AFTER `global_key` already matched '1'..'5',
            // which would mis-route digits typed into a query. Block
            // that here by capturing every printable char first.
            Some(Action::SearchEditAppend(c))
        }
        _ => None,
    }
}

fn detail_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => Some(Action::Back),
        _ => None,
    }
}

fn simple_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn kc(c: char, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), m)
    }

    fn special(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_c_always_quits() {
        let s = AppState::new();
        assert_eq!(
            handle_key(&s, kc('c', KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
    }

    #[test]
    fn q_quits_on_non_search_panes() {
        let s = AppState::new();
        assert_eq!(handle_key(&s, k('q')), Some(Action::Quit));
    }

    #[test]
    fn digit_hotkeys_switch_panes_outside_search() {
        let mut s = AppState::new();
        s.current = Pane::Home;
        assert_eq!(
            handle_key(&s, k('2')),
            Some(Action::SwitchPane(Pane::Browse))
        );
        s.current = Pane::Browse;
        assert_eq!(handle_key(&s, k('1')), Some(Action::SwitchPane(Pane::Home)));
        assert_eq!(
            handle_key(&s, k('5')),
            Some(Action::SwitchPane(Pane::Stats))
        );
    }

    #[test]
    fn slash_in_browse_goes_to_search() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        assert_eq!(
            handle_key(&s, k('/')),
            Some(Action::SwitchPane(Pane::Search))
        );
    }

    #[test]
    fn jk_navigate_lists() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        assert_eq!(handle_key(&s, k('j')), Some(Action::Down));
        assert_eq!(handle_key(&s, k('k')), Some(Action::Up));
        assert_eq!(handle_key(&s, k('g')), Some(Action::Top));
        assert_eq!(handle_key(&s, k('G')), Some(Action::Bottom));
    }

    #[test]
    fn enter_in_list_opens_detail() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        assert_eq!(
            handle_key(&s, special(KeyCode::Enter)),
            Some(Action::OpenDetail)
        );
    }

    #[test]
    fn esc_in_detail_goes_back() {
        let mut s = AppState::new();
        s.current = Pane::Detail;
        assert_eq!(handle_key(&s, special(KeyCode::Esc)), Some(Action::Back));
    }

    #[test]
    fn search_pane_consumes_printable_chars() {
        let mut s = AppState::new();
        s.current = Pane::Search;
        // Letters: appended.
        assert_eq!(handle_key(&s, k('a')), Some(Action::SearchEditAppend('a')));
        // Digit '2' is a pane hotkey globally BUT in Search it must
        // go into the query so people can search for "v0.2".
        // Global handler currently fires first; the test documents
        // current behavior: digit jumps to pane.
        //
        // Adjusted: route Search input first inside handle_key().
        // See the special-case branch added below in handle_key.
    }

    #[test]
    fn search_pane_swallows_digits_too() {
        // Routing fix: when Search is active, digits must NOT switch
        // panes — they must go into the query.
        let mut s = AppState::new();
        s.current = Pane::Search;
        assert_eq!(
            handle_key(&s, k('2')),
            Some(Action::SearchEditAppend('2')),
            "digit while typing search must append to query, not switch pane"
        );
    }

    #[test]
    fn search_enter_submits() {
        let mut s = AppState::new();
        s.current = Pane::Search;
        assert_eq!(
            handle_key(&s, special(KeyCode::Enter)),
            Some(Action::SearchSubmit)
        );
    }
}
