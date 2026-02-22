//! Terminal search functionality for Termy
//!
//! This crate provides fast, native search capabilities for terminal buffer content.
//! It supports both literal and regex search modes with case sensitivity options.
//!
//! # Example
//!
//! ```
//! use termy_search::{SearchState, SearchMatch};
//!
//! let mut state = SearchState::new();
//! state.open();
//! state.set_query("error");
//!
//! // Search terminal content (line provider returns line text)
//! state.search(-100, 24, |line_idx| {
//!     // Return line text from terminal buffer
//!     Some(format!("Line {}: some error here", line_idx))
//! });
//!
//! // Navigate results
//! if let Some((current, total)) = state.results().position() {
//!     println!("Match {} of {}", current, total);
//! }
//! state.next_match();
//! ```

mod engine;
mod matcher;
mod state;

pub use engine::{SearchConfig, SearchEngine, SearchMode};
pub use matcher::{SearchMatch, SearchResults};
pub use state::SearchState;
