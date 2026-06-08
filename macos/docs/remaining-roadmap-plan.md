# Remaining roadmap — execution-ready plans

After this session the roadmap is complete except for **one multi-week feature**
(tmux control mode) and **one infra-gated step** (P6's GPU frame-time comparison).
Both are scoped below. Verified against the source on 2026-06-08.

> Items previously listed here that are now **done**: line-mark scrollbar markers
> (cap-aware command marks, `CommandMarkTests`) and the native P6 benchmark with
> render-plan build-time percentiles (`TermyBenchmarkRunner`).

---

## 1. tmux control mode (M3) — multi-week, architecturally gated

**Why it isn't done:** a control parser exists (`crates/terminal_ui/src/tmux/control/parser.rs`),
but it lives in `terminal_ui`, which the FFI crate is **boundary-forbidden** to
depend on (`scripts/check-boundaries.sh`: `check_forbidden_dep "termy_ffi" "termy_terminal_ui"`).
So the native host can't reuse it, and full control mode is essentially a tmux
client (window/pane model, layout sync, transitions) — weeks of work.

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
   - **Remaining (GUI-gated last mile):** a `TmuxControlSession` that maps the
     parsed tmux layout ↔ a `TerminalPane` tree, routes `%output` into panes,
     forwards input, and handles native↔tmux transitions, behind a config flag
     (per-session shell exec stays the fallback). **Pane rendering is the only
     part that needs the macOS GUI to validate.**

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
