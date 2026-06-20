# Remaining roadmap — execution-ready plans

The native roadmap is complete except for **one GUI-gated tmux step** and **one
infra-gated step** (P6's GPU frame-time comparison). Both are scoped below.

---

## 1. tmux control mode (M3) — GUI wiring remaining

**Why it isn't done:** the tmux protocol, FFI surface, Swift wrapper, layout
parser, display-terminal mode, and `TmuxControlSession` orchestration are done
and verified. The remaining work is presentation: render the reconciled tmux
layout in SwiftUI, route focused-pane input through `sendInput(toPane:)`, and
ship it behind a config flag while the per-pane shell fallback remains available.

**Plan:**
1. ✅ **Done.** Extracted the tmux control protocol (parser/state-machine,
   coalescer, channel/worker, command, payload, types) from
   `crates/terminal_ui/src/tmux` into the new shared `crates/tmux_control_core`
   (only `flume` + `std`; no GPUI, no `terminal_ui`). `terminal_ui` consumes it
   and is behavior-unchanged — verified: 34 moved protocol tests + 121
   `terminal_ui` lib tests + 8 tmux integration tests (real tmux 3.6b) pass, the
   GPUI app builds, boundaries pass. The `termy_ffi → terminal_ui` boundary no
   longer blocks the native host.
2. ✅ **Control session core done & verified.** `tmux_control_core::session::ControlSession`
   opens a PTY (`rustix-openpty`, unix), spawns `tmux -CC new-session`, runs the
   control worker, and exposes `poll()` / `send_command()` (5s-bounded) /
   `shutdown()`. Tested against **real tmux 3.6b** (`#[ignore]`d like the other
   tmux tests): launches, receives parsed startup notifications, completes a
   command round-trip, tears down. Additive — the GPUI app is untouched.
2b. ✅ **Done & verified.** `termy_tmux_control_{open,poll,send,close}` +
   `TermyFfiTmuxNotification`/`...Batch` in `crates/ffi` wrap `ControlSession`,
   tested end-to-end against real tmux (`crates/ffi/tests/tmux_control_ffi.rs`).
   The whole Rust + FFI backend for native tmux control mode is complete.
3. **Native host (step 3).**
   - ✅ **FFI wrapper done & verified.** `Support/LibTermyTmuxControl.swift` wraps
     `open`/`poll`/`send`/`close` and decodes the C notification batch into a Swift
     `TmuxControlNotification`; tested against real tmux in the macOS test target
     (`LibTermyTmuxControlTests`, skips cleanly without tmux).
   - ✅ **Layout parsing done & verified.** `Support/TmuxLayout.swift` parses tmux
     `#{window_layout}` strings into a pane tree (single/h-split/v-split/nested);
     `TmuxLayoutTests`.
   - ✅ **Display terminal done & verified.** tmux panes display `%output` rather
     than running a shell, so `termy_core` gained a PTY-less terminal mode
     (`Terminal::new_display` + `feed_output`; `pty_tx` is now Optional, `write` a
     no-op). Exposed via FFI (`termy_display_terminal_new`,
     `termy_terminal_feed_output`) and Swift (`LibTermyTerminal(displayCols:…)` +
     `feedOutput`). Verified headlessly (fed bytes land in the grid — ffi + Swift
     tests) with the existing PTY terminal unchanged (161 core tests).
   - ✅ **Orchestration done & verified.** `TmuxControlSession` reconciles the
     parsed `TmuxLayout` into display terminals (create/resize/remove per pane),
     routes each `%output` to its pane, and forwards input via hex `send-keys`.
     Verified end-to-end against real tmux (`TmuxControlSessionTests`: split →
     two display terminals → shell `%output` lands in a pane grid).
   - **Remaining (GUI-gated):** render `TmuxControlSession.layout` +
     `terminal(forPane:)` in the SwiftUI workspace — place pane grids per the
     layout tree (h/v splits), wire keyboard input to `sendInput(toPane:)`, behind
     a config flag (per-session shell exec stays the fallback). Everything below
     the view layer is built and verified; only on-screen rendering needs the GUI.

**Estimate:** the entire stack — protocol → `ControlSession` → FFI → Swift wrapper
→ layout parsing — is **done and verified against real tmux**. Only the GUI pane
rendering/wiring remains.

---

## 2. P6 — GPU frame-time gate against the GPUI baseline — needs benchmark runs

**Done:** `TermySwift --benchmark` emits render-plan rebuild metrics **and**
build-time percentiles (`native-build-times {p50,p95,p99,…}`).

**Remaining (needs a machine that can run both apps under `xctrace`):**
1. Add a *windowed* benchmark mode that measures real GPU present times (the
   headless run only times CPU render-plan builds).
2. `xtask benchmark-compare`: add the native app as a target driven via
   `TERMY_BENCHMARK_COMMAND` (`TermySwift --benchmark`), parse the percentiles,
   and compare against the GPUI baseline with the existing no-regression
   thresholds.
3. Wire into `scripts/check-performance-gates.sh` / CI.
