//! Stats pane — fuller numeric view than Home, includes sessions.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title("stats");
    let Some(s) = &state.stats else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  (no stats yet — press 'r' to refresh)",
                Style::default().fg(theme::DIM),
            )))
            .block(block),
            area,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // observation totals
            Constraint::Length(6), // session totals
            Constraint::Min(0),    // breakdowns
        ])
        .split(area);

    let totals = vec![
        Line::from(Span::styled("observations", theme::header())),
        Line::from(format!("  active:        {}", s.observations.active)),
        Line::from(format!("  soft-deleted:  {}", s.observations.deleted)),
        Line::from(format!("  projects:      {}", s.observations.projects)),
    ];
    f.render_widget(
        Paragraph::new(totals).block(Block::default().borders(Borders::ALL).title("observations")),
        chunks[0],
    );

    let sessions = vec![
        Line::from(Span::styled("sessions", theme::header())),
        Line::from(format!("  total:         {}", s.sessions.total)),
    ];
    f.render_widget(
        Paragraph::new(sessions).block(Block::default().borders(Borders::ALL).title("sessions")),
        chunks[1],
    );

    let bk = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let by_type: Vec<Line> = s
        .observations
        .by_type
        .iter()
        .map(|b| Line::from(format!("  {:<14} {}", b.key, b.count)))
        .collect();
    f.render_widget(
        Paragraph::new(by_type).block(Block::default().borders(Borders::ALL).title("by type")),
        bk[0],
    );

    let by_scope: Vec<Line> = s
        .observations
        .by_scope
        .iter()
        .map(|b| Line::from(format!("  {:<14} {}", b.key, b.count)))
        .collect();
    f.render_widget(
        Paragraph::new(by_scope).block(Block::default().borders(Borders::ALL).title("by scope")),
        bk[1],
    );
}
