use std::collections::HashSet;

/// A single match in the terminal buffer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line index (negative values = scrollback history)
    pub line: i32,
    /// Start column (inclusive)
    pub start_col: usize,
    /// End column (exclusive)
    pub end_col: usize,
}

impl SearchMatch {
    pub fn new(line: i32, start_col: usize, end_col: usize) -> Self {
        Self {
            line,
            start_col,
            end_col,
        }
    }

    /// Check if a cell position is within this match
    pub fn contains(&self, line: i32, col: usize) -> bool {
        self.line == line && col >= self.start_col && col < self.end_col
    }
}

/// Container for search results with navigation
#[derive(Debug, Clone)]
pub struct SearchResults {
    matches: Vec<SearchMatch>,
    current_index: Option<usize>,
    /// Pre-computed set of (line, col) pairs for O(1) match lookups during rendering
    match_cells: HashSet<(i32, usize)>,
}

impl Default for SearchResults {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchResults {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            current_index: None,
            match_cells: HashSet::new(),
        }
    }

    pub fn from_matches(matches: Vec<SearchMatch>) -> Self {
        let current_index = if matches.is_empty() { None } else { Some(0) };
        // Pre-compute all match cells for O(1) lookup during rendering
        let match_cells = Self::build_match_cells(&matches);
        Self {
            matches,
            current_index,
            match_cells,
        }
    }

    /// Build a HashSet of all (line, col) pairs that are part of any match
    fn build_match_cells(matches: &[SearchMatch]) -> HashSet<(i32, usize)> {
        let mut cells = HashSet::new();
        for m in matches {
            for col in m.start_col..m.end_col {
                cells.insert((m.line, col));
            }
        }
        cells
    }

    /// Total number of matches
    pub fn count(&self) -> usize {
        self.matches.len()
    }

    /// Check if there are any matches
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Get all matches
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// Get the currently focused match
    pub fn current(&self) -> Option<&SearchMatch> {
        self.current_index.and_then(|i| self.matches.get(i))
    }

    /// Get current position as (current_1_indexed, total)
    pub fn position(&self) -> Option<(usize, usize)> {
        self.current_index.map(|i| (i + 1, self.matches.len()))
    }

    /// Move to the next match (wraps around)
    pub fn next(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        let next_index = match self.current_index {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        };
        self.current_index = Some(next_index);
        self.matches.get(next_index)
    }

    /// Move to the previous match (wraps around)
    pub fn previous(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        let prev_index = match self.current_index {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.matches.len() - 1,
        };
        self.current_index = Some(prev_index);
        self.matches.get(prev_index)
    }

    /// Jump to a specific match index
    pub fn jump_to(&mut self, index: usize) -> Option<&SearchMatch> {
        if index < self.matches.len() {
            self.current_index = Some(index);
            self.matches.get(index)
        } else {
            None
        }
    }

    /// Find the match nearest to a given line (for initial positioning)
    pub fn jump_to_nearest(&mut self, target_line: i32) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }

        // Find first match at or after target_line
        let index = self
            .matches
            .iter()
            .position(|m| m.line >= target_line)
            .unwrap_or(0);

        self.current_index = Some(index);
        self.matches.get(index)
    }

    /// Check if a cell is part of the current match
    pub fn is_current_match(&self, line: i32, col: usize) -> bool {
        self.current()
            .map(|m| m.contains(line, col))
            .unwrap_or(false)
    }

    /// Check if a cell is part of any match (O(1) lookup)
    pub fn is_any_match(&self, line: i32, col: usize) -> bool {
        self.match_cells.contains(&(line, col))
    }

    /// Get matches visible in a viewport range
    pub fn matches_in_range(&self, min_line: i32, max_line: i32) -> Vec<&SearchMatch> {
        self.matches
            .iter()
            .filter(|m| m.line >= min_line && m.line <= max_line)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_match_contains() {
        let m = SearchMatch::new(5, 10, 15);
        assert!(m.contains(5, 10));
        assert!(m.contains(5, 14));
        assert!(!m.contains(5, 15)); // exclusive end
        assert!(!m.contains(5, 9));
        assert!(!m.contains(4, 12));
    }

    #[test]
    fn test_empty_results() {
        let results = SearchResults::new();
        assert!(results.is_empty());
        assert_eq!(results.count(), 0);
        assert!(results.current().is_none());
        assert!(results.position().is_none());
    }

    #[test]
    fn test_navigation() {
        let matches = vec![
            SearchMatch::new(0, 0, 5),
            SearchMatch::new(1, 10, 15),
            SearchMatch::new(2, 5, 10),
        ];
        let mut results = SearchResults::from_matches(matches);

        assert_eq!(results.position(), Some((1, 3)));
        assert_eq!(results.current().unwrap().line, 0);

        results.next();
        assert_eq!(results.position(), Some((2, 3)));
        assert_eq!(results.current().unwrap().line, 1);

        results.next();
        assert_eq!(results.position(), Some((3, 3)));

        // Wrap around
        results.next();
        assert_eq!(results.position(), Some((1, 3)));

        // Previous
        results.previous();
        assert_eq!(results.position(), Some((3, 3)));
    }

    #[test]
    fn test_jump_to_nearest() {
        let matches = vec![
            SearchMatch::new(-10, 0, 5),
            SearchMatch::new(-5, 0, 5),
            SearchMatch::new(0, 0, 5),
            SearchMatch::new(5, 0, 5),
        ];
        let mut results = SearchResults::from_matches(matches);

        results.jump_to_nearest(-7);
        assert_eq!(results.current().unwrap().line, -5);

        results.jump_to_nearest(0);
        assert_eq!(results.current().unwrap().line, 0);

        results.jump_to_nearest(100);
        assert_eq!(results.current().unwrap().line, -10); // wraps to first
    }
}
