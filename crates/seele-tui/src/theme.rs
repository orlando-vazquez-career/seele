//! Hardcoded dark theme (ADR-07 §Theming). v0.2 may load from config.

use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;
pub const ERR: Color = Color::Red;

pub fn selected() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn header() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn footer_hint() -> Style {
    Style::default().fg(DIM)
}
