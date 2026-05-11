//! View rendering. `render` chooses one of five sub-renderers based
//! on the current pane and lays out a global header + footer around
//! the body region.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppState, Pane};
use crate::theme;

mod browse;
mod detail;
mod home;
mod search;
mod stats;

pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer hint
        ])
        .split(area);

    render_header(f, chunks[0], state);
    match state.current {
        Pane::Home => home::render(f, chunks[1], state),
        Pane::Browse => browse::render(f, chunks[1], state),
        Pane::Search => search::render(f, chunks[1], state),
        Pane::Detail => detail::render(f, chunks[1], state),
        Pane::Stats => stats::render(f, chunks[1], state),
    }
    render_footer(f, chunks[2], state);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let panes = [
        Pane::Home,
        Pane::Browse,
        Pane::Search,
        Pane::Detail,
        Pane::Stats,
    ];
    let mut spans = vec![Span::styled(
        format!("SEELE v{}  ", env!("CARGO_PKG_VERSION")),
        theme::header(),
    )];
    for p in panes {
        let style = if p == state.current {
            theme::selected()
        } else {
            Style::default().fg(theme::DIM)
        };
        spans.push(Span::styled(
            format!("[{}] {} ", p.hotkey(), p.label()),
            style,
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_footer(f: &mut Frame, area: Rect, state: &AppState) {
    let hint = match state.current {
        Pane::Home => "[1-5] panes  [r] refresh  [q] quit",
        Pane::Browse => "[j/k] move  [enter] open  [/] search  [r] refresh  [q] quit",
        Pane::Search => "[type] query  [enter] run  [esc] clear  [j/k] move  [q] quit",
        Pane::Detail => "[esc/h/⌫] back  [q] quit",
        Pane::Stats => "[r] refresh  [q] quit",
    };
    let mut line = vec![Span::styled(hint, theme::footer_hint())];
    if let Some(err) = &state.last_error {
        line.push(Span::raw("  "));
        line.push(Span::styled(
            format!("error: {err}"),
            Style::default().fg(theme::ERR),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(line)), area);
}
