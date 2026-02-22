use crate::engine::{SearchConfig, SearchEngine, SearchMode};
use crate::matcher::SearchResults;

/// Manages the search session lifecycle
pub struct SearchState {
    engine: SearchEngine,
    results: SearchResults,
    query: String,
    is_active: bool,
    error: Option<String>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            engine: SearchEngine::new(SearchConfig::default()),
            results: SearchResults::new(),
            query: String::new(),
            is_active: false,
            error: None,
        }
    }

    /// Activate search mode
    pub fn open(&mut self) {
        self.is_active = true;
    }

    /// Deactivate search mode and clear results
    pub fn close(&mut self) {
        self.is_active = false;
        self.clear();
    }

    /// Check if search is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Get current query string
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Update the search query
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        match self.engine.set_pattern(query) {
            Ok(()) => self.error = None,
            Err(e) => self.error = Some(e),
        }
    }

    /// Clear search state
    pub fn clear(&mut self) {
        self.query.clear();
        let _ = self.engine.set_pattern("");
        self.results = SearchResults::new();
        self.error = None;
    }

    /// Get current search results
    pub fn results(&self) -> &SearchResults {
        &self.results
    }

    /// Get mutable search results (for navigation)
    pub fn results_mut(&mut self) -> &mut SearchResults {
        &mut self.results
    }

    /// Get any regex compilation error
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Check if there's a valid pattern to search
    pub fn has_valid_pattern(&self) -> bool {
        self.engine.has_pattern()
    }

    /// Toggle case sensitivity
    pub fn toggle_case_sensitive(&mut self) {
        let mut config = self.config();
        config.case_sensitive = !config.case_sensitive;
        self.engine.set_config(config);
    }

    /// Toggle regex mode
    pub fn toggle_regex_mode(&mut self) {
        let mut config = self.config();
        config.mode = match config.mode {
            SearchMode::Literal => SearchMode::Regex,
            SearchMode::Regex => SearchMode::Literal,
        };
        self.engine.set_config(config);
        // Recompile pattern with new mode
        let query = self.query.clone();
        self.set_query(&query);
    }

    /// Get current config
    pub fn config(&self) -> SearchConfig {
        SearchConfig {
            case_sensitive: self.is_case_sensitive(),
            mode: self.mode(),
        }
    }

    /// Check if case sensitive search is enabled
    pub fn is_case_sensitive(&self) -> bool {
        // Access through engine's config (would need getter)
        // For now, track separately or access engine internals
        false // TODO: expose from engine
    }

    /// Get current search mode
    pub fn mode(&self) -> SearchMode {
        SearchMode::Literal // TODO: expose from engine
    }

    /// Execute search on terminal content
    pub fn search<F>(&mut self, start_line: i32, end_line: i32, line_provider: F)
    where
        F: Fn(i32) -> Option<String>,
    {
        self.results = self.engine.search(start_line, end_line, line_provider);
    }

    /// Navigate to next match
    pub fn next_match(&mut self) {
        self.results.next();
    }

    /// Navigate to previous match
    pub fn previous_match(&mut self) {
        self.results.previous();
    }

    /// Jump to match nearest to a line
    pub fn jump_to_nearest(&mut self, line: i32) {
        self.results.jump_to_nearest(line);
    }
}
