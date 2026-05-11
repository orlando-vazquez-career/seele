//! Search pane — query prompt + result list.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_query(f, chunks[0], state);
    render_results(f, chunks[1], state);
}

fn render_query(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title("query");
    let mut spans = vec![Span::raw(state.search_query.clone())];
    spans.push(Span::styled("█", Style::default().fg(theme::ACCENT)));
    f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn render_results(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("results ({})", state.search_results.len()));

    if state.search_results.is_empty() {
        let lines = vec![Line::from(Span::styled(
            "  (type a query and press Enter)",
            Style::default().fg(theme::DIM),
        ))];
        f.render_widget(Paragraph::new(lines).block(block), area);
        return;
    }

    let items: Vec<ListItem> = state
        .search_results
        .iter()
        .map(|h| {
            let spans = vec![
                Span::styled(short_id(&h.id), Style::default().fg(theme::DIM)),
                Span::raw("  "),
                Span::styled(
                    format!("★{:.2}", h.score),
                    Style::default().fg(score_color(h.score)),
                ),
                Span::raw("  "),
                Span::raw(format!("[{}]", h.project.as_deref().unwrap_or("-"))),
                Span::raw("  "),
                Span::raw(truncate(&h.title, 60)),
            ];
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected());
    let mut ls = ListState::default();
    ls.select(Some(
        state
            .selected
            .min(state.search_results.len().saturating_sub(1)),
    ));
    f.render_stateful_widget(list, area, &mut ls);
}

fn short_id(id: &str) -> String {
    if id.len() > 10 {
        format!("{}…", &id[..10])
    } else {
        id.to_string()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    } else {
        s.to_string()
    }
}

fn score_color(score: f64) -> ratatui::style::Color {
    use ratatui::style::Color;
    if score >= 0.7 {
        Color::Green
    } else if score >= 0.4 {
        Color::Yellow
    } else {
        Color::DarkGray
    }
}
