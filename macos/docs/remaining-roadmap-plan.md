# Completed implementation plans

Status date: 2026-07-10

This file is retained as a historical pointer. Its former tmux GUI plan is
complete, and its former P6 plan has been absorbed into the production roadmap.
The active release checklist is [`../road.md`](../road.md).

## tmux control mode

The complete control-mode path is implemented:

1. `tmux_control_core` owns the parser, state machine, worker, commands, and
   control session.
2. `termy_ffi` exposes control-session open, poll, send, and close operations.
3. The Swift host wraps the FFI session and parses tmux layouts.
4. Display-only libtermy terminals receive each pane's `%output` stream.
5. `TmuxControlWorkspaceModel` reconciles layouts, output, focus, input, search,
   split, and close commands.
6. `TmuxControlWorkspaceView` recursively renders horizontal and vertical pane
   layouts and routes keyboard, mouse, and paste input to the focused pane.
7. The shell-backed `TmuxIntegration` path remains as a launch fallback.

The remaining tmux work is production QA through real AppKit events and failure
paths, tracked by Task 5 in the active roadmap.

## Native performance comparison

The headless native benchmark and CPU render-plan percentile gate are
implemented. Sustained scenario coverage, statistically meaningful sample
counts, resource budgets, and the windowed GPUI comparison are tracked together
by Task 4 in the active roadmap.
