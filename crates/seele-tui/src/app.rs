//! TUI application state. Owns the current pane, list selections,
//! search query, and cached fetch results. All side-effecting calls
//! into the service live here so `lib.rs::apply_action` can stay a
//! thin dispatch table.

use seele_core::id::SeeleId;
use seele_http::dto::{ObservationDto, SearchHitDto, StatsResponse};
use seele_http::service::enforce_search_query_or_filter;
use seele_http::{
    dto::{ListRequest, SearchRequest},
    SeeleService,
};

/// The five top-level panes from ADR-07.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Home,
    Browse,
    Search,
    Detail,
    Stats,
}

impl Pane {
    pub fn label(&self) -> &'static str {
        match self {
            Pane::Home => "Home",
            Pane::Browse => "Browse",
            Pane::Search => "Search",
            Pane::Detail => "Detail",
            Pane::Stats => "Stats",
        }
    }

    pub fn hotkey(&self) -> char {
        match self {
            Pane::Home => '1',
            Pane::Browse => '2',
            Pane::Search => '3',
            Pane::Detail => '4',
            Pane::Stats => '5',
        }
    }
}

pub struct AppState {
    pub current: Pane,
    /// Pane the user was on before opening Detail. `Esc` on Detail
    /// returns to this pane so a Search → Detail → back trip lands
    /// back on Search instead of always on Browse.
    pub prev_pane: Option<Pane>,
    /// Set to `true` on `q` (or `Ctrl-C`) to break the event loop.
    pub should_quit: bool,
    /// Cached stats — refreshed when Home / Stats is opened.
    pub stats: Option<StatsResponse>,
    /// Cached browse list.
    pub browse: Vec<ObservationDto>,
    /// Selected index inside the active list pane (browse OR search).
    pub selected: usize,
    /// Active free-text query in Search pane.
    pub search_query: String,
    /// Live search results.
    pub search_results: Vec<SearchHitDto>,
    /// Observation displayed on Detail pane.
    pub detail: Option<ObservationDto>,
    /// Last error to surface in the footer.
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current: Pane::Home,
            prev_pane: None,
            should_quit: false,
            stats: None,
            browse: Vec::new(),
            selected: 0,
            search_query: String::new(),
            search_results: Vec::new(),
            detail: None,
            last_error: None,
        }
    }

    pub fn switch_pane(&mut self, p: Pane) {
        // Resetting selection on pane switch avoids landing on an
        // out-of-range index when the new pane has fewer rows.
        if p != self.current {
            self.selected = 0;
        }
        self.current = p;
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.current_list_len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let max = len.saturating_sub(1);
        let cur = self.selected as i32;
        let next = (cur + delta).clamp(0, max as i32) as usize;
        self.selected = next;
    }

    pub fn jump_to(&mut self, idx: usize) {
        let len = self.current_list_len();
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = idx.min(len - 1);
        }
    }

    pub fn current_list_len(&self) -> usize {
        match self.current {
            Pane::Browse => self.browse.len(),
            Pane::Search => self.search_results.len(),
            _ => 0,
        }
    }

    /// Force-refresh everything used by Home (stats, top-of-list
    /// browse). Errors are surfaced via `last_error`; nothing the TUI
    /// loop can recover from is propagated.
    pub fn refresh(&mut self, service: &SeeleService) {
        self.refresh_stats(service);
        self.refresh_browse(service);
    }

    /// Refresh only what the active pane displays. Used on pane
    /// switch to keep Home/Stats numbers fresh without re-issuing
    /// the same SQL on every keystroke.
    pub fn refresh_for_current_pane(&mut self, service: &SeeleService) {
        match self.current {
            Pane::Home => {
                self.refresh_stats(service);
                self.refresh_browse(service);
            }
            Pane::Browse => self.refresh_browse(service),
            Pane::Stats => self.refresh_stats(service),
            Pane::Search | Pane::Detail => {}
        }
    }

    fn refresh_stats(&mut self, service: &SeeleService) {
        match service.stats() {
            Ok(s) => {
                self.stats = Some(s);
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(format!("stats: {e}"));
            }
        }
    }

    fn refresh_browse(&mut self, service: &SeeleService) {
        let req = ListRequest {
            project: None,
            scope: None,
            r#type: None,
            topic_key: None,
            session_id: None,
            limit: Some(BROWSE_LIMIT),
            include_deleted: false,
        };
        match service.list_observations(req) {
            Ok(rows) => {
                self.browse = rows;
                if self.selected >= self.browse.len() && !self.browse.is_empty() {
                    self.selected = self.browse.len() - 1;
                }
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(format!("list: {e}")),
        }
    }

    pub fn run_search(&mut self, service: &SeeleService) {
        let req = SearchRequest {
            query: self.search_query.clone(),
            project: None,
            scope: None,
            r#type: None,
            limit: Some(BROWSE_LIMIT),
            include_purist: false,
            include_annotations: false,
            // Canonical default (ADR-16 D3); 0.0 would disable the boost.
            score_boost_multiplier: 1.0,
            max_vec_distance: None,
        };
        if let Err(e) = enforce_search_query_or_filter(&req) {
            self.last_error = Some(format!("search: {e:?}"));
            return;
        }
        match service.search_observations(req) {
            Ok(resp) => {
                self.search_results = resp.hits;
                self.selected = 0;
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(format!("search: {e}")),
        }
    }

    pub fn open_detail_for_selection(&mut self, service: &SeeleService) {
        let (origin, id_str) = match self.current {
            Pane::Browse => (
                Pane::Browse,
                self.browse.get(self.selected).map(|o| o.id.clone()),
            ),
            Pane::Search => (
                Pane::Search,
                self.search_results.get(self.selected).map(|h| h.id.clone()),
            ),
            _ => return,
        };
        let Some(id_str) = id_str else { return };
        // Parse failure is a soft error: a corrupt id in the DB
        // surfaces in the footer instead of kicking Orlando out of
        // the TUI. Closes Cloven 2026-05-11 [MEDIO]: was propagated
        // through apply_action, terminating the event loop.
        let id: SeeleId = match id_str.parse() {
            Ok(id) => id,
            Err(e) => {
                self.last_error = Some(format!("bad id '{id_str}': {e}"));
                return;
            }
        };
        match service.get_observation(id) {
            Ok(Some(o)) => {
                self.detail = Some(o);
                self.prev_pane = Some(origin);
                self.current = Pane::Detail;
            }
            Ok(None) => self.last_error = Some(format!("observation {id_str} not found")),
            Err(e) => self.last_error = Some(format!("show: {e}")),
        }
    }

    /// Esc on Detail returns to the pane the user opened it from
    /// (Browse or Search). Falls back to Browse when no origin was
    /// recorded.
    pub fn back(&mut self) {
        if self.current == Pane::Detail {
            self.current = self.prev_pane.take().unwrap_or(Pane::Browse);
        }
    }

    /// Esc on Search: when the query has content, clear it; when the
    /// query is already empty, exit Search and go to Browse. Gives
    /// Orlando a deterministic way out of Search without Ctrl-C
    /// (Cloven 2026-05-11 [MEDIO]).
    pub fn search_escape(&mut self) {
        if self.search_query.is_empty() && self.search_results.is_empty() {
            self.current = Pane::Browse;
        } else {
            self.search_query.clear();
            self.search_results.clear();
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

const BROWSE_LIMIT: u32 = 50;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_on_home_with_no_quit() {
        let s = AppState::new();
        assert_eq!(s.current, Pane::Home);
        assert!(!s.should_quit);
        assert!(s.stats.is_none());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn switch_pane_resets_selection() {
        let mut s = AppState::new();
        s.selected = 7;
        s.switch_pane(Pane::Browse);
        assert_eq!(s.selected, 0);
        assert_eq!(s.current, Pane::Browse);
    }

    #[test]
    fn switch_pane_to_same_keeps_selection() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        s.selected = 3;
        s.switch_pane(Pane::Browse);
        assert_eq!(s.selected, 3);
    }

    #[test]
    fn move_selection_clamps_to_zero_when_empty() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        s.move_selection(-5);
        assert_eq!(s.selected, 0);
        s.move_selection(3);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn move_selection_clamps_at_bounds() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        // Synthesize 3 entries by direct field set; we're testing
        // clamp behavior, not data flow.
        s.browse = (0..3)
            .map(|_| ObservationDto {
                id: "01HW".to_string(),
                session_id: None,
                r#type: "memory".to_string(),
                title: "t".to_string(),
                content: "c".to_string(),
                tool_name: None,
                project: None,
                scope: "project".to_string(),
                topic_key: None,
                revision_count: 0,
                duplicate_count: 0,
                last_seen_at: 0,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                metadata: serde_json::Value::Null,
            })
            .collect();
        s.move_selection(10);
        assert_eq!(s.selected, 2);
        s.move_selection(-10);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn search_escape_clears_query_when_non_empty() {
        // Cloven 2026-05-11 [MEDIO]: first Esc clears, only the
        // empty-query Esc exits to Browse.
        let mut s = AppState::new();
        s.current = Pane::Search;
        s.search_query = "rate limit".to_string();
        s.search_escape();
        assert_eq!(s.current, Pane::Search);
        assert_eq!(s.search_query, "");
    }

    #[test]
    fn search_escape_exits_to_browse_when_empty() {
        let mut s = AppState::new();
        s.current = Pane::Search;
        s.search_escape();
        assert_eq!(s.current, Pane::Browse);
    }

    #[test]
    fn back_from_detail_returns_to_origin_pane() {
        // Cloven 2026-05-11 [MEDIO]: prev_pane records where Detail
        // was opened from so a Search → Detail → Esc lands back on
        // Search, not always on Browse.
        let mut s = AppState::new();
        s.prev_pane = Some(Pane::Search);
        s.current = Pane::Detail;
        s.back();
        assert_eq!(s.current, Pane::Search);
        assert!(s.prev_pane.is_none(), "prev_pane should be consumed");
    }

    #[test]
    fn back_falls_back_to_browse_when_no_origin_recorded() {
        let mut s = AppState::new();
        s.current = Pane::Detail;
        s.back();
        assert_eq!(s.current, Pane::Browse);
    }

    #[test]
    fn jump_to_bottom_lands_on_last_index() {
        let mut s = AppState::new();
        s.current = Pane::Browse;
        s.browse = (0..4)
            .map(|_| ObservationDto {
                id: "01HW".to_string(),
                session_id: None,
                r#type: "memory".to_string(),
                title: "t".to_string(),
                content: "c".to_string(),
                tool_name: None,
                project: None,
                scope: "project".to_string(),
                topic_key: None,
                revision_count: 0,
                duplicate_count: 0,
                last_seen_at: 0,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                metadata: serde_json::Value::Null,
            })
            .collect();
        s.jump_to(usize::MAX);
        assert_eq!(s.selected, 3);
        s.jump_to(0);
        assert_eq!(s.selected, 0);
    }
}
