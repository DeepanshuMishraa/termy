//! Shared tmux control-mode (`tmux -CC`) protocol core: command construction,
//! payload (un)escaping, the control-stream parser/state machine, notification
//! coalescing, and the worker channel plumbing. Extracted from `terminal_ui` so
//! both the GPUI app and the FFI/native host can drive tmux control mode.

pub mod command;
pub mod control;
pub mod payload;
pub mod session;
pub mod types;
