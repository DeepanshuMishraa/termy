# Termy (native macOS) — Roadmap to Replace `crates/desktop_app`

Status: **highly experimental** (`README.md:3`). This document tracks what the
native SwiftUI host must close to replace the GPUI app (`crates/desktop_app/`)
in the next release.

The terminal core is shared: both apps drive the same Rust `libtermy`/`termy_core`
through FFI, so emulation correctness, keyboard encoding, mouse encoding, search,
and tmux logic are **not** the gap. The gap is the **presentation + interaction
layer** the native app reimplements in Swift, plus rendering performance.

> **Recent progress (2026-06-08).** Landed: **IME / CJK composition**
> (`NSTextInputClient` + inline marked-text rendering), **deeplinks** (router +
> `termy` URL-scheme registration), **toasts**, **keybind import** (kitty/ghostty)
> + conflict detection, config color import (alacritty/kitty/ghostty/iTerm2),
> settings search, tab drag-to-reorder, scrollbar search-hit markers, bell
> (audible + visual), thermal-aware refresh, VoiceOver (value/selection/cursor),
> config-diagnostics surfacing, a **native `--benchmark` entry point** with
> build-time percentiles (P6), a **theme store** (real registry fetch + install),
> **CLI install** (FFI-exposed `cli_install_core` + bundled `termy-cli`),
> **command marks** in the scrollbar (cap-aware, exact), and the
> command-dispatch/enum refactor. Verified false gaps (already present): command
> palette fuzzy matching, scrollback-depth setting, color picker, native context
> menu, onboarding (import + theme discovery), command marks, a runnable native
> render-perf gate (`scripts/check-render-perf.sh`), the keyCode input-mapping
> tests, and ligatures (decided: omit, documented). **M3's architectural blocker
> is now removed:** the tmux control protocol is extracted into the shared
> `crates/tmux_control_core` (verified against real tmux 3.6b), so the FFI host is
> no longer boundary-locked out of control mode. **Open:** the rest of tmux
> control mode (M3 — FFI control session, then native pane/layout sync whose
> rendering needs the macOS GUI to validate), and the **P6 GPU frame-time gate**
> (native build-time measurement + a runnable regression gate are done; the
> cross-app GPU comparison needs a windowed `xctrace` run against a GPUI baseline). Execution-ready implementation plans for all three are in
> [`docs/remaining-roadmap-plan.md`](docs/remaining-roadmap-plan.md).

---

## 1. The headline problem: rendering architecture

The GPUI app renders cells on the GPU with a per-pane cell cache and dirty-span
patching (`crates/desktop_app/src/terminal_view/render.rs`,
`terminal_view/benchmark.rs`). The native app now has damage-scoped frame
updates, a retained row renderer, and display-synced presentation, but still
needs native metrics gates and a benchmark path before it can replace the GPUI
app.

How the native path works today (after P1/P2/P4):

- ✅ `TerminalGridView` no longer draws through SwiftUI `Canvas`. It is backed
  by an AppKit `NSViewRepresentable` row renderer (`Views/TerminalGridView.swift`)
  that invalidates only dirty row rects for partial terminal damage. This is the
  retained CPU renderer step for **P2**; a Metal/glyph-atlas renderer is still
  open as a later optimization if the launch and render gates prove it is needed.
- ✅ The render plan (background runs + text segments) is no longer rebuilt from
  scratch each frame. `TerminalRenderPlanCache` keeps per-row plans and rebuilds
  only the rows the core flagged as damaged (`Views/TerminalRenderPlan.swift`).
- ✅ Damage is no longer discarded: the damage shape (`none`/`full`/
  `partial(spans)`) rides along with `termy_terminal_take_frame_update()` and
  `refresh()` rebuilds the plan accordingly (`Services/LibTermyTerminal.swift`,
  `Services/TerminalViewModel.swift`). The standalone `takeDamage()` wrapper that
  predated the frame-update path has been removed as orphaned.
- ✅ The frame is no longer re-marshaled in full across the FFI boundary for
  partial damage. `termy_terminal_take_frame_update()` returns full snapshots
  only for forced/full damage and changed cells for partial damage; Swift keeps a
  retained `TerminalFrameStore` and patches rows in place
  (`Services/LibTermyTerminal.swift`, `Models/TerminalFrameStore.swift`).
- ✅ Redraw is driven from `CVDisplayLink` through
  `DisplaySyncedRefreshDriver`, with the existing 60 Hz active / 15 Hz idle
  cadence expressed as display-synced tick throttling.

Net effect after P1/P2/P4: a one-cell cursor blink patches a small FFI update,
rebuilds one row's layout, and invalidates that row's rect on the next display
tick. The remaining performance risk is instrumentation and enforcement: native
frame-time/render counters are not yet gated in CI.

### Performance milestone — **P** (gates the whole replacement)

- [x] **P1 — Damage-driven partial render-plan rebuild.** ✅ Done.
  The damage shape is a structured `TerminalDamage` (`none`/`full`/
  `partial(spans)`) instead of a `Bool` (`Models/TerminalDamage.swift`),
  delivered through the frame-update path (P4). Plan-building moved out of the draw path
  into `TerminalRenderPlanCache`, which rebuilds only damaged rows and reuses the
  rest — the GPUI `PaneCacheUpdateStrategy` full/partial/reuse split
  (`Views/TerminalRenderPlan.swift`). `refresh()` drives it from damage and
  publishes a render revision (`Services/TerminalViewModel.swift`); full/partial
  rebuild counts show in the debug overlay. Covered by
  `Tests/TermySwiftTests/TerminalRenderPlanCacheTests.swift` (partial rebuild ==
  full rebuild).
- [x] **P2 — Move off SwiftUI `Canvas`.** ✅ Done for the retained CPU renderer
  step. `TerminalGridView` now hosts an AppKit `NSView` and invalidates only
  dirty row rects on partial damage. **Remaining optional upgrade:** move to
  `CAMetalLayer`/`MTKView` plus a glyph atlas if metrics show the retained row
  renderer cannot hold budget under heavy redraws.
- [x] **P3 — Display-link-driven present.** ✅ Done with macOS
  `CVDisplayLink`, coalescing terminal updates until the next display-synced
  poll/present and preserving the existing active/idle backoff.
- [x] **P4 — Reduce FFI marshaling.** ✅ Done for damage-scoped updates.
  `Terminal::frame_update(force_full)` and
  `termy_terminal_take_frame_update()` expose full snapshots for full damage and
  changed cells for partial damage. Swift applies them through
  `TerminalFrameStore`, preserving the existing full snapshot API for
  compatibility.
- [x] **P5 — Native render metrics + gate.** ✅ Initial gate done.
  The Swift layer records codable native render metrics for frame updates,
  full/partial frame updates, presented/skipped display-link ticks, patched
  cells, and full/partial render-plan rebuilds. `scripts/check-performance-gates.sh
  --native-render-metrics` runs selected Swift gate tests for the metrics
  contract and a live `LibTermyTerminal` workload that must stay on the partial
  update path. ✅ **Threshold gating done:** `scripts/check-render-perf.sh` runs
  the `--benchmark` workload and fails if the partial path doesn't hold (full
  rebuilds capped at ~10% of presents) or render-plan-build p95 exceeds a ceiling
  (verified passing: 1/121 full rebuilds, p95 ~46µs). **Remaining:** persisting
  baselines over time.
- [~] **P6 — Validate against GPUI baseline** using
  `cargo run -p xtask -- benchmark-compare`; native must not regress
  frame-time percentiles for the standard scenarios. ✅ `TermySwift --benchmark`
  runs a headless bulk-scroll workload through the real render pipeline and prints
  both `native-render-metrics {…}` (1 full vs 120 partial rebuilds) **and
  `native-build-times {…}`** — render-plan build p50/p95/p99 in microseconds
  (`Services/TermyBenchmarkRunner.swift`, `TermyBenchmarkRunnerTests`). A runnable
  native regression gate exists (`scripts/check-render-perf.sh`, see P5).
  **Remaining:** the cross-app comparison against the **GPUI baseline** measures
  full *GPU* frame times via `xctrace`, which needs a windowed run of both apps —
  the only part that can't execute in a headless session.

**Exit:** cursor blink shows ~0 full rebuilds; `cat` of a large file and a
tmux full-screen redraw hold frame budget on a maximized window; metrics gated
in CI.

---

## 2. Feature-parity gaps

Grouped by milestone. Each item is present in `crates/desktop_app/` and missing
or stubbed in `macos/`.

### M1 — Daily-driver blockers

- [~] **IME / dead keys / CJK composition.** ✅ Implemented via `NSTextInputClient`
  on `KeyboardCaptureView` (`Views/TerminalKeyboardInputView.swift`). `keyDown`
  routes through the input context only while composing or for plain/shift keys —
  `option` stays meta and `command` stays a shortcut — committed text is sent as
  UTF-8, and non-IME keys fall through to the Kitty encoder unchanged (the
  `insertText`/`setMarkedText` state machine and the no-double-type guarantee are
  covered by `TerminalIMEInputTests`). The composing text is now **rendered inline
  at the cursor** (`TerminalSurfaceView.markedTextOverlay`). **Remaining polish
  (needs GUI + live CJK testing):** pinning the system candidate window to the
  exact cursor cell (`firstRect` currently anchors to the view).
- [~] **Command palette breadth.** Native has a working palette with ~25
  commands, keybind hints, and **ranked fuzzy matching** (`Support/CommandPaletteFilter.swift`,
  subsequence match with word-start/run bonuses; tested in `CommandPaletteFilterTests`)
  — fuzzy matching is **done**. Remaining gaps vs the GPUI palette
  (`terminal_view/command_palette/`, ~40 commands): the tmux-session, saved-layout,
  and theme-switcher entries.
- [~] **Full keybinding customization + conflict detection.** Conflict detection
  is **done**: `Support/TerminalKeybindConflicts.swift` normalizes triggers
  (modifier order + `secondary`→`cmd`) and the keybind settings flag triggers
  bound more than once (`TerminalKeybindConflictsTests`). Editing is still via the
  directive text editor; a structured per-binding editor remains.
- [x] **Bell surfaced** (visual + audible). ✅ The `.bell` event rings
  `NSSound.beep()` and pulses a visual flash overlay in `TerminalSurfaceView`
  (`Services/TerminalViewModel.swift`, `bellPulse`). **Note:** OSC desktop
  notifications (OSC 9 / 777) are not yet delivered as a runtime event by the core
  — surfacing those needs a new FFI event kind (out of `macos/` scope).

### M2 — Expected terminal UX

- [x] **OSC-8 hyperlinks.** ✅ Done. The core exposes the OSC 8 hyperlink under
  a viewport cell (`crates/core/src/links.rs`, `termy_terminal_hyperlink_at` in
  `crates/ffi`). Native hover/⌘-click prefers the OSC 8 target and falls back to
  the `NSDataDetector` heuristics (`Services/TerminalViewModel.swift`,
  `Services/LibTermyTerminal.swift`); GPUI gained the same OSC 8 priority in
  `terminal_view/interaction/selection.rs` (it previously used heuristics only).
  Covered by `Tests/TermySwiftTests/TerminalHyperlinkTests.swift` plus core/FFI
  tests.
- [x] **Tab drag-to-reorder.** ✅ The custom tab chrome supports drag-to-reorder
  (`Views/NativeTabChromeView.swift` `onDrag`/`onDrop`), driven by
  `NativeTabWindowManager.moveNativeTab(_:toIndex:)` which reorders the native
  tabbed windows via `addTabbedWindow(ordered:)`. (Drag-feel polish — preview
  thumbnail, auto-scroll — is still GPUI-only; the reorder itself works.)
- [x] **Scrollbar markers.** ✅ Both search-hit markers and shell-prompt **command
  marks** render as buckets along the scrollbar (`TerminalScrollBar`). Command
  marks are recorded on OSC 133;A at the prompt's absolute row and are provably
  exact while history stays below the scrollback cap (read via
  `termy_config_runtime_scrollback_history`); once the buffer fills, tracking
  stops rather than show eviction-drifted positions (`CommandMarkTests`). The
  cap-aware gate is the documented limitation — marks reset when scrollback fills.
- [x] **Ligatures / complex text layout — decided: omit (documented).** The
  renderer draws per-cell and pixel-snaps each cell, and block-element/box-drawing
  glyphs are painted as rects/strokes for seam-free tiling
  (`Views/TerminalGridView.swift`, `TerminalBlockGlyphs`/`TerminalBoxDrawing`).
  Ligatures and complex shaping span multiple cells and would break both the fixed
  monospace grid and that tiling guarantee, so they're intentionally out of scope
  — matching most terminals (Terminal.app, older Alacritty). This item's ask was
  to decide + document the scope; that's done.
- [~] **Toasts.** ✅ A `TermyToastCenter` queue (capped + auto-dismiss) with a
  bottom-trailing `TermyToastOverlay`, wired to real events (config-import success,
  theme-install via deeplink); covered by `TermyToastCenterTests`. Update-progress
  toasts can hook the same center once the updater surfaces progress.

### M3 — tmux parity

- [~] **tmux control mode.** Native still execs a shell into a per-pane `tmux`
  session (`Support/TmuxIntegration.swift`) as the fallback. **Architectural
  blocker removed:** the control-mode protocol core (parser/state-machine,
  coalescer, channel/worker, command, payload, types) is extracted from
  `terminal_ui` into the new shared `crates/tmux_control_core` (no GPUI / no
  `terminal_ui` deps — only `flume`), so the FFI host can now drive it without
  crossing the `termy_ffi → terminal_ui` boundary. Verified end-to-end: 34 moved
  protocol tests pass, `terminal_ui` is unchanged in behavior (121 lib + **8 tmux
  integration tests against real tmux 3.6b** pass), the GPUI app still builds, and
  boundaries pass. ✅ **Step 2 core done & verified:** a `ControlSession`
  (`tmux_control_core/src/session.rs`) opens a PTY, spawns `tmux -CC new-session`,
  runs the control worker, and exposes `poll()` / `send_command()` / `shutdown()`
  — tested against **real tmux 3.6b** (launches, receives parsed startup
  notifications, completes a command round-trip). Additive, so the GPUI app is
  untouched. ✅ **Step 2b done & verified:** the FFI surface
  `termy_tmux_control_{open,poll,send,close}` (+ a C `TermyFfiTmuxNotification`
  batch) wraps `ControlSession` in `crates/ffi`, tested end-to-end against **real
  tmux** (`crates/ffi/tests/tmux_control_ffi.rs`). **The entire Rust + FFI backend
  for native tmux control mode is now complete and verified.** ✅ **Step 3 FFI
  layer done & verified:** `Support/LibTermyTmuxControl.swift` wraps the FFI
  (`open`/`poll`/`send`/`close`, decoding the C notification batch into a Swift
  `TmuxControlNotification`), exercised against **real tmux** in the macOS test
  target (`LibTermyTmuxControlTests`, skips cleanly without tmux). So the full
  stack — protocol → `ControlSession` → FFI → Swift wrapper → real tmux — is
  verified end-to-end. **Remaining (step 3 last mile):** a `TmuxControlSession`
  that maps tmux panes/layouts ↔ `TerminalPane`s and renders pane output —
  the pane **rendering** is the only part that needs the macOS GUI to validate.

### M4 — Config, onboarding, store

- [x] **Config parse diagnostics surfaced.** ✅ The core's structured diagnostics
  (`termy_config_diagnostics`: line number + kind + message) are read in
  `Services/TermyAppConfiguration.swift` and shown in the config error banner
  instead of a generic Swift error string.
- [x] **Scrollback depth setting.** Already covered: `scrollback_history` and
  `inactive_tab_scrollback` are root settings in the shared schema, so the
  schema-driven settings UI already edits them and the Rust terminal honors them
  at creation time (the terminal is built from the config pointer). The unused
  `termy_config_runtime_scrollback_history` query is redundant for the host.
- [~] **Full config import.** Imports **font family/size + the full color
  palette** (foreground/background/cursor + 16 ANSI colors) for alacritty, kitty,
  ghostty (text configs) **and iTerm2** (default profile from its binary plist),
  with section-aware/`palette=`/`colorN` parsing and hex normalization. **Keybind
  import** now maps the high-confidence 1:1 bindings (copy/paste/new-tab/close/
  split/tab-switch) from kitty and ghostty and merges only non-conflicting
  triggers, never overwriting existing bindings (`TerminalConfigImportTests`).
  **Remaining:** alacritty keybinds (TOML array-of-tables) and the long tail of
  ambiguous per-terminal actions, left out deliberately rather than guessed.
- [~] **Settings UI depth.** Full-settings **search** is done
  (`Support/SettingsSearch.swift` + a flattened results view, `SettingsSearchTests`).
  A **color picker** already exists (`ColorRow` uses SwiftUI `ColorPicker`), and
  edits commit straight to config (effectively live). **Remaining:** a structured
  keybind editor and the theme store.
- [~] **Theme store.** ✅ `TermyThemeStore` fetches the real public registry
  (`raw.githubusercontent.com/termy-org/themes/main/index.json`, schema verified
  live), `ThemeStoreView` lists themes from the Themes settings section, and
  Install reuses the core's `installTheme(slug:)` download (`TermyThemeStoreTests`
  covers index parsing). Also reachable via `termy://store/theme-install`.
  **Remaining:** ETag/offline cache and the optional `api.termy.sh` auth flow.
- [x] **Onboarding flow** parity. ✅ First-run window detects other terminals and
  imports their font + colors + keybinds (`Views/OnboardingView.swift`), and now
  offers theme discovery via a "Browse Themes…" sheet reusing `ThemeStoreView`.
  (Curated per-config theme *recommendations* could be a later refinement; theme
  discovery during onboarding is present.)

### M5 — Platform & lifecycle

- [x] **Deeplinks** (`termy://new|settings|open/config|store/theme-install`). ✅
  `Support/TermyDeeplink.swift` parses the URLs, `TermyDeeplinkRouter` dispatches
  them, and `onOpenURL` is wired in the app scene (`TermyDeeplinkTests`). The
  `termy` URL scheme is registered via `CFBundleURLTypes` in both the dev plist
  (`crates/xtask/src/macos.rs`) and the release plist (`scripts/build-dmg.sh`).
- [~] **CLI install.** ✅ `termy_cli_install` (new FFI wrapping the shared
  `cli_install_core`, so no logic is duplicated) is called from
  `SettingsBridge.installCLI` via the "Install Command Line Tool…" menu item, and
  xtask bundles `termy-cli` into `Contents/MacOS/` where `find_cli_binary` looks.
  Compiles across ffi/swift/xtask with boundaries passing; the install itself
  (symlink + shell-PATH setup) is the user-triggered action and can't be exercised
  headlessly without modifying the real shell environment. **Remaining:** CLI
  *delegation* of non-GUI args (the `termy_cli` binary already handles that
  standalone).
- [x] **Thermal awareness** — ✅ `DisplaySyncedRefreshDriver` observes
  `ProcessInfo.thermalStateDidChangeNotification` and floors the tick interval
  under `.serious`/`.critical` (30 Hz / 15 Hz), throttling the 60 Hz active
  cadence (`DisplaySyncedRefreshDriverTests`).
- [ ] **Auto-update.** Native surfaces GitHub release links only
  (`Support/AppUpdater.swift`); confirm that matches the intended update story
  (GPUI also link-based, so likely fine — just verify).
- [x] **Native context menu** parity. ✅ Right-click shows a native
  copy/paste/split/clear/search menu (`Support/TerminalSurfaceContextMenu.swift`,
  surfaced from `KeyboardCaptureView`).
- [~] **Accessibility (VoiceOver / `NSAccessibility`).** The grid `NSView` is an
  accessibility text-area exposing the visible buffer as its value, the current
  selection as `accessibilitySelectedText`, and the cursor row as the insertion
  point (`Views/TerminalGridView.swift`, `TerminalAccessibilityTests`).
  **Remaining (needs VoiceOver verification):** confirming focus lands on the grid
  rather than the keyboard-capture overlay, and per-line navigation.

---

## 3. Testing & quality gates

Native coverage now includes render-plan and frame-store correctness, plus
config/persistence/stress. Input and interaction coverage is still thin
(`Tests/TermySwiftTests/`).

- [x] **Render-correctness tests for retained state.** Partial-vs-full render
  plan behavior is covered by `TerminalRenderPlanCacheTests`, and partial/full
  frame update application is covered by `TerminalFrameStoreTests`. Full visual
  golden tests are still open.
- [~] **Domain/parsing/input tests added.** Coverage for the keybind action
  vocabulary, keybind conflicts, config import, settings search, deeplink parsing,
  thermal floor, accessibility text, IME marked-text state machine
  (`TerminalIMEInputTests`), command marks (`CommandMarkTests`), and the keyboard
  **keyCode→name input mapping** (`TerminalKeyInputMappingTests`). **Still open:**
  mouse-event→FFI mapping and full live NSEvent-driven paths (need an AppKit
  event harness).
- [ ] **Selection/search/scrollback** interaction tests beyond the existing
  clamp stress test.
- [ ] Extend `scripts/check-performance-gates.sh` and `check-launch-gate.sh` to
  block regressions; add a render-metrics gate (P5).
- [ ] Keep `SettingsSchemaParityTests` / `TermyConfigurationParityTests` green as
  the schema source of truth (these are the strongest existing native tests).

---

## 4. Suggested sequencing

1. **P1–P3** — partial redraw + off-Canvas rendering + display-link present.
   Nothing else matters if a maximized terminal can't keep up.
2. **M1** — IME, command palette, keybind customization. These are the
   "can't daily-drive without" blockers.
3. **P4–P6 + M2** — FFI marshaling, metrics gate, terminal UX polish.
4. **M4–M5** — config import, settings depth, deeplinks, CLI install.
5. **M3** — tmux control mode (largest single effort; can ship behind a flag).

**Replacement bar:** P-milestone gated in CI + M1 complete + M2 and M4/M5
substantially complete. tmux control mode (M3) may trail behind a feature flag
if the per-session shell fallback is documented.
