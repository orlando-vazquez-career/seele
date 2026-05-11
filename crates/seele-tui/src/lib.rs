//! SEELE TUI — interactive terminal UI with ratatui (ADR-07).
//!
//! Five views (Home, Browse, Search, Detail, Stats) navigable with
//! vi-style keys (`j/k`, `/`, `1-5`, `q`). The TUI talks to
//! `seele_http::SeeleService` directly — no HTTP round trip, no MCP
//! envelope. Same in-process write path the CLI uses.
//!
//! Entrypoint: `run_tui(service)`. Wires crossterm raw mode + alt
//! screen, drives a tokio `select!` loop that reads keystrokes and
//! re-renders. Restoration of the terminal is deferred via a `Drop`
//! guard so panics still hand the terminal back cleanly.

use std::io::{self, Stdout};

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use seele_http::SeeleService;

mod app;
mod event;
mod theme;
mod views;

pub use app::{AppState, Pane};
pub use event::{handle_key, Action};

/// Re-export the top-level view renderer so integration / snapshot
/// tests can drive it against a `TestBackend` without spinning up
/// the event loop.
pub fn views_render(f: &mut ratatui::Frame, state: &AppState) {
    views::render(f, state);
}

/// Public crate-level error type.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("storage: {0}")]
    Storage(#[from] seele_storage::StorageError),
    #[error("service: {0}")]
    Service(String),
}

pub type Result<T> = std::result::Result<T, TuiError>;

/// Run the TUI against `service` until the user quits. Wires up the
/// crossterm backend, enters the alternate screen, and ensures we
/// restore the terminal even on panic.
pub async fn run_tui(service: SeeleService) -> Result<()> {
    let mut terminal = enter_tui()?;
    let _guard = TerminalGuard;

    let mut state = AppState::new();
    // Warm the initial pane so Home has stats to show.
    state.refresh(&service)?;

    run_event_loop(&mut terminal, &service, &mut state).await?;

    Ok(())
}

fn enter_tui() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// RAII guard that restores the terminal when dropped. The unwraps
/// are intentional: terminal restoration is best-effort, and a
/// panicking guard inside `Drop` is worse than a silent restore.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

async fn run_event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    service: &SeeleService,
    state: &mut AppState,
) -> Result<()> {
    use crossterm::event::EventStream;
    use futures::StreamExt;

    let mut events = EventStream::new();
    loop {
        terminal.draw(|f| views::render(f, state))?;

        let Some(event_res) = events.next().await else {
            break;
        };
        let evt = match event_res {
            Ok(e) => e,
            Err(_) => continue,
        };
        if let crossterm::event::Event::Key(key) = evt {
            if let Some(action) = handle_key(state, key) {
                apply_action(state, service, action)?;
            }
        }
        if state.should_quit {
            break;
        }
    }
    Ok(())
}

/// Dispatch an `Action` against the service + state. Pure-ish (no
/// IO) actions live in `app.rs`; this function is the side-effecting
/// boundary.
pub fn apply_action(state: &mut AppState, service: &SeeleService, action: Action) -> Result<()> {
    match action {
        Action::Quit => state.should_quit = true,
        Action::SwitchPane(p) => {
            state.switch_pane(p);
            // Refreshing on switch keeps Home/Browse/Stats numbers
            // current without a separate refresh keybinding.
            state.refresh_for_current_pane(service)?;
        }
        Action::Up => state.move_selection(-1),
        Action::Down => state.move_selection(1),
        Action::Top => state.jump_to(0),
        Action::Bottom => state.jump_to(usize::MAX),
        Action::Refresh => state.refresh(service)?,
        Action::SearchEditAppend(c) => state.search_query.push(c),
        Action::SearchEditBackspace => {
            state.search_query.pop();
        }
        Action::SearchSubmit => state.run_search(service)?,
        Action::SearchClear => {
            state.search_query.clear();
            state.search_results.clear();
        }
        Action::OpenDetail => state.open_detail_for_selection(service)?,
        Action::Back => state.back_to_browse(),
    }
    Ok(())
}
