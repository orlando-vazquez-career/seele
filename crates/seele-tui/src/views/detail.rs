//! Detail pane — single observation rendered with metadata + links.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use chrono::TimeZone;

use crate::app::AppState;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(o) = &state.detail else {
        let block = Block::default().borders(Borders::ALL).title("detail");
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  (no observation selected — pick one in browse/search and press Enter)",
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
            Constraint::Length(7), // header card
            Constraint::Min(4),    // body
            Constraint::Length(5), // metadata
        ])
        .split(area);

    render_header_card(f, chunks[0], o);
    render_body(f, chunks[1], o);
    render_metadata(f, chunks[2], o);
}

fn render_header_card(f: &mut Frame, area: Rect, o: &seele_http::dto::ObservationDto) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("memory {}", o.id));
    let lines = vec![
        Line::from(format!("  Created:   {}", ms_to_human(o.created_at))),
        Line::from(format!("  Updated:   {}", ms_to_human(o.updated_at))),
        Line::from(format!(
            "  Project:   {}",
            o.project.as_deref().unwrap_or("-")
        )),
        Line::from(format!("  Scope:     {}", o.scope)),
        Line::from(format!(
            "  Topic key: {}",
            o.topic_key.as_deref().unwrap_or("-")
        )),
    ];
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_body(f: &mut Frame, area: Rect, o: &seele_http::dto::ObservationDto) {
    let block = Block::default().borders(Borders::ALL).title("body");
    let title_line = Line::from(Span::styled(o.title.clone(), theme::header()));
    let mut lines = vec![title_line, Line::from("")];
    for body_line in o.content.lines() {
        lines.push(Line::from(body_line.to_string()));
    }
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_metadata(f: &mut Frame, area: Rect, o: &seele_http::dto::ObservationDto) {
    let block = Block::default().borders(Borders::ALL).title("metadata");
    let pretty = serde_json::to_string_pretty(&o.metadata).unwrap_or_else(|_| "{}".to_string());
    f.render_widget(
        Paragraph::new(pretty)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn ms_to_human(ms: i64) -> String {
    chrono::Utc
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| ms.to_string())
}
