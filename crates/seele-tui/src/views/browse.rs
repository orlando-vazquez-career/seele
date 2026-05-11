//! Browse pane — flat list of recent observations, j/k navigable.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("browse ({} active)", state.browse.len()));

    if state.browse.is_empty() {
        let empty = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
            "  (no observations — save one with `seele save <title> <content>` and press 'r')",
            Style::default().fg(theme::DIM),
        )))
        .block(block);
        f.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .browse
        .iter()
        .map(|o| {
            let spans = vec![
                Span::styled(short_id(&o.id), Style::default().fg(theme::DIM)),
                Span::raw("  "),
                Span::raw(format!("[{}]", o.project.as_deref().unwrap_or("-"))),
                Span::raw("  "),
                Span::raw(truncate(&o.title, 60)),
            ];
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected());
    let mut ls = ListState::default();
    ls.select(Some(
        state.selected.min(state.browse.len().saturating_sub(1)),
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
