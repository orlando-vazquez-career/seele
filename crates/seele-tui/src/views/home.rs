//! Home pane — stats summary + navigation hints. The widget shape
//! mirrors ADR-07's mock: a Stats card, a "By kind" / "By scope"
//! breakdown row, and the pane chooser at the bottom (which already
//! lives in the global header).

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // stats card
            Constraint::Min(0),    // breakdowns
        ])
        .split(area);

    render_stats_card(f, chunks[0], state);
    render_breakdowns(f, chunks[1], state);
}

fn render_stats_card(f: &mut Frame, area: Rect, state: &AppState) {
    let mut lines = vec![Line::from(Span::styled(
        "📊 Stats",
        Style::default().fg(theme::ACCENT),
    ))];
    match &state.stats {
        Some(s) => {
            lines.push(Line::from(format!(
                "  Total memories: {}",
                s.observations.active + s.observations.deleted
            )));
            lines.push(Line::from(format!(
                "  Active:         {}",
                s.observations.active
            )));
            lines.push(Line::from(format!(
                "  Soft-deleted:   {}",
                s.observations.deleted
            )));
            lines.push(Line::from(format!(
                "  Projects:       {}",
                s.observations.projects
            )));
        }
        None => {
            lines.push(Line::from(Span::styled(
                "  (no stats yet — press 'r' to refresh)",
                Style::default().fg(theme::DIM),
            )));
        }
    }
    let block = Block::default().borders(Borders::ALL).title("home");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_breakdowns(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_kind_breakdown(f, chunks[0], state);
    render_scope_breakdown(f, chunks[1], state);
}

fn render_kind_breakdown(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title("by kind");
    let lines: Vec<Line> = match &state.stats {
        Some(s) => s
            .observations
            .by_type
            .iter()
            .map(|b| Line::from(format!("  {:<14} {}", b.key, b.count)))
            .collect(),
        None => vec![Line::from(Span::styled(
            "  (no data)",
            Style::default().fg(theme::DIM),
        ))],
    };
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_scope_breakdown(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title("by scope");
    let lines: Vec<Line> = match &state.stats {
        Some(s) => s
            .observations
            .by_scope
            .iter()
            .map(|b| Line::from(format!("  {:<14} {}", b.key, b.count)))
            .collect(),
        None => vec![Line::from(Span::styled(
            "  (no data)",
            Style::default().fg(theme::DIM),
        ))],
    };
    f.render_widget(Paragraph::new(lines).block(block), area);
}
