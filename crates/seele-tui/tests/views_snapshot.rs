//! Snapshot tests for the five TUI views. Each test builds an
//! `AppState`, renders it onto a fixed-width `TestBackend` (120×30),
//! and compares against an `insta` snapshot.
//!
//! Width is pinned so CI on Linux/Mac/Windows produces identical
//! frame buffers — terminals that auto-resize would otherwise make
//! the snapshots noisy.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use seele_http::dto::{
    CountBucket, ObservationDto, ObservationStats, SearchHitDto, SessionStats, StatsResponse,
};
use seele_tui::{AppState, Pane};

fn fresh_terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(120, 30)).unwrap()
}

fn buffer_dump(term: &Terminal<TestBackend>) -> String {
    let backend = term.backend();
    let buf = backend.buffer();
    let area = buf.area;
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buf[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}

fn stub_stats() -> StatsResponse {
    StatsResponse {
        observations: ObservationStats {
            active: 12,
            deleted: 2,
            projects: 3,
            by_type: vec![
                CountBucket {
                    key: "memory".to_string(),
                    count: 8,
                },
                CountBucket {
                    key: "decision".to_string(),
                    count: 4,
                },
            ],
            by_scope: vec![
                CountBucket {
                    key: "project".to_string(),
                    count: 10,
                },
                CountBucket {
                    key: "personal".to_string(),
                    count: 2,
                },
            ],
        },
        sessions: SessionStats {
            total: 1,
            by_status: vec![],
        },
    }
}

fn stub_observation(id: &str, title: &str) -> ObservationDto {
    ObservationDto {
        id: id.to_string(),
        session_id: None,
        r#type: "memory".to_string(),
        title: title.to_string(),
        content: "stub content\nsecond line".to_string(),
        tool_name: None,
        project: Some("seele".to_string()),
        scope: "project".to_string(),
        topic_key: None,
        revision_count: 0,
        duplicate_count: 0,
        last_seen_at: 0,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
        deleted_at: None,
        metadata: serde_json::json!({"kind": "memory"}),
    }
}

#[test]
fn home_renders_stats_and_breakdowns() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Home;
    state.stats = Some(stub_stats());

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn browse_renders_list_with_selection_at_zero() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Browse;
    state.browse = vec![
        stub_observation("01HW000000000000000000000A", "alpha"),
        stub_observation("01HW000000000000000000000B", "beta"),
        stub_observation("01HW000000000000000000000C", "gamma"),
    ];
    state.selected = 0;

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn browse_empty_state_renders_hint() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Browse;
    state.browse = vec![];

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn search_renders_query_and_results() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Search;
    state.search_query = "rate limit".to_string();
    state.search_results = vec![SearchHitDto {
        id: "01HW000000000000000000000X".to_string(),
        title: "stripe rate-limit policy".to_string(),
        content: "exponential backoff".to_string(),
        project: Some("seele".to_string()),
        scope: "project".to_string(),
        r#type: "decision".to_string(),
        score: 0.95,
        fts_rank: Some(1),
        vec_rank: None,
        created_at: 1_700_000_000_000,
        metadata: serde_json::Value::Null,
        annotations: vec![],
    }];

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn search_empty_state_prompts_for_query() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Search;

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn detail_renders_observation_card_body_metadata() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Detail;
    state.detail = Some(stub_observation("01HWDETAIL000000000000000", "deep dive"));

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn detail_empty_state_when_no_observation_selected() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Detail;
    state.detail = None;

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn stats_renders_full_breakdown() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Stats;
    state.stats = Some(stub_stats());

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn stats_empty_state_renders_hint() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Stats;
    state.stats = None;

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}

#[test]
fn last_error_surfaces_in_footer() {
    let mut term = fresh_terminal();
    let mut state = AppState::new();
    state.current = Pane::Home;
    state.last_error = Some("boom".to_string());

    term.draw(|f| seele_tui::views_render(f, &state)).unwrap();
    insta::assert_snapshot!(buffer_dump(&term));
}
