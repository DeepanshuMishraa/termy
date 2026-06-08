# tmux_control_core

Shared, UI-agnostic core for tmux **control mode** (`tmux -CC`): command-line
construction, payload (un)escaping, the control-stream parser/state machine,
notification coalescing, and the worker channel plumbing.

## Boundary

Pure protocol logic — depends only on `flume` and `std`. It has **no GPUI, no
app/UI, and no `terminal_ui` dependency**, so it can be driven by both
`terminal_ui` (the GPUI app) and the FFI/native host (`termy_ffi`). Extracted
from `terminal_ui::tmux` so the native macOS host can implement tmux control mode
without crossing the `termy_ffi → terminal_ui` boundary.

Process spawning, PTY wiring, and pane/layout integration live in the consumer
(e.g. `terminal_ui::tmux::client`), not here.
