# TermyAlpha (native macOS) — Roadmap to Replace `crates/desktop_app`

Status: **highly experimental** (`README.md:3`). This document tracks what the
native SwiftUI host must close to replace the GPUI app (`crates/desktop_app/`)
in the next release.

The terminal core is shared: both apps drive the same Rust `libtermy`/`termy_core`
through FFI, so emulation correctness, keyboard encoding, mouse encoding, search,
and tmux logic are **not** the gap. The gap is the **presentation + interaction
layer** the native app reimplements in Swift, plus rendering performance.

---

## 1. The headline problem: rendering architecture

The GPUI app renders cells on the GPU with a per-pane cell cache and dirty-span
patching (`crates/desktop_app/src/terminal_view/render.rs`,
`terminal_view/benchmark.rs`). The native app does **none** of that.

How the native path works today (after P1):

- `TerminalGridView` draws into a **SwiftUI `Canvas`** with
  `rendersAsynchronously: false` — CPU rasterization through `GraphicsContext`,
  no Metal, no `CALayer` reuse (`Views/TerminalGridView.swift`). The `Canvas`
  cannot be partially invalidated, so it still repaints the **whole** grid every
  frame even when one cell changed. → **P2**.
- ✅ The render plan (background runs + text segments) is no longer rebuilt from
  scratch each frame. `TerminalRenderPlanCache` keeps per-row plans and rebuilds
  only the rows the core flagged as damaged (`Views/TerminalRenderPlan.swift`).
- ✅ Damage is no longer discarded: `takeDamage()` returns a structured
  `TerminalDamage` (`none`/`full`/`partial(spans)`) and `refresh()` rebuilds the
  plan accordingly (`Services/LibTermyTerminal.swift`,
  `Services/TerminalViewModel.swift`).
- The frame is still re-marshaled in full across the FFI boundary on every
  update: `snapshot()` allocates a Swift array and `.map`s **every cell**
  (`Services/LibTermyTerminal.swift`). → **P4**.
- Redraw is timer-driven at 60 Hz active / 15 Hz idle, not `CADisplayLink`-
  synced (`Services/TerminalViewModel.swift`). → **P3**.

Net effect after P1: a one-cell cursor blink rebuilds only one row's layout, but
still triggers a full-grid FFI marshal **and** a full-grid CPU repaint. Until P2
and P4 land, this will still struggle with `yes`, `cat largefile`, `tmux`
redraws, or a fast `tput` loop on a maximized window.

### Performance milestone — **P** (gates the whole replacement)

- [x] **P1 — Damage-driven partial render-plan rebuild.** ✅ Done.
  `takeDamage()` now returns a structured `TerminalDamage` (`none`/`full`/
  `partial(spans)`) instead of a `Bool` (`Models/TerminalDamage.swift`,
  `Services/LibTermyTerminal.swift`). Plan-building moved out of the draw path
  into `TerminalRenderPlanCache`, which rebuilds only damaged rows and reuses the
  rest — the GPUI `PaneCacheUpdateStrategy` full/partial/reuse split
  (`Views/TerminalRenderPlan.swift`). `refresh()` drives it from damage and
  publishes a render revision (`Services/TerminalViewModel.swift`); full/partial
  rebuild counts show in the debug overlay. Covered by
  `Tests/TermySwiftTests/TerminalRenderPlanCacheTests.swift` (partial rebuild ==
  full rebuild). **Remaining for full P1:** the SwiftUI `Canvas` still repaints
  the whole grid — partial *paint* (vs partial plan rebuild) needs P2.
- [ ] **P2 — Move off SwiftUI `Canvas`.** Render via a Metal-backed layer
  (`CAMetalLayer`/`MTKView`) or at minimum a retained `CALayer`/`CATextLayer`
  grid that reuses glyph runs. Glyph atlas + per-cell cache, not per-frame
  `GraphicsContext` paths.
- [ ] **P3 — `CADisplayLink`-driven present** synced to the display refresh,
  coalescing damage between vsyncs, with the existing active/idle backoff.
- [ ] **P4 — Reduce FFI marshaling.** Avoid allocating a fresh cell array per
  frame. Reuse buffers, or expose a damage-scoped snapshot that returns only
  changed cells.
- [ ] **P5 — Native render metrics + gate.** Port `TERMY_RENDER_METRICS`-style
  `full`/`partial`/`reuse` counters into the Swift layer and wire them into
  `scripts/check-performance-gates.sh` so regressions fail CI.
- [ ] **P6 — Validate against GPUI baseline** using
  `cargo run -p xtask -- benchmark-compare`; native must not regress
  frame-time percentiles for the standard scenarios.

**Exit:** cursor blink shows ~0 full rebuilds; `cat` of a large file and a
tmux full-screen redraw hold frame budget on a maximized window; metrics gated
in CI.

---

## 2. Feature-parity gaps

Grouped by milestone. Each item is present in `crates/desktop_app/` and missing
or stubbed in `macos/`.

### M1 — Daily-driver blockers

- [ ] **IME / dead keys / CJK composition.** No marked-text handling in
  `KeyboardCaptureView` (`Views/TerminalKeyboardInputView.swift`). GPUI has full
  IME with marked ranges + candidate bounds (`terminal_view/inline_input.rs`,
  `text_input.rs`). Blocks any non-Latin input.
- [ ] **Command palette breadth + fuzzy matching.** Native already has a working
  palette with ~25 commands, substring filtering, and keybind hints
  (`Views/TerminalWorkspaceView.swift:289-444`) — *not* a stub. Gaps vs the GPUI
  palette (`terminal_view/command_palette/`, ~40 commands): fuzzy/ranked matching
  instead of substring `.filter`, plus the tmux-session, saved-layout, and
  theme-switcher entries.
- [ ] **Full keybinding customization + conflict detection.** GPUI resolves
  user keybinds with platform awareness and conflict detection
  (`keybindings/mod.rs`, `commands.rs`, `settings_view/keybinds.rs`); native
  relies on fixed menu shortcuts.
- [ ] **Bell / OSC notifications surfaced** (visual/audible). Not handled in the
  native event loop.

### M2 — Expected terminal UX

- [ ] **OSC-8 hyperlinks.** Native link detection is `NSDataDetector` heuristics
  only (`Support/TerminalLinkInteraction.swift:14-39`); GPUI supports real OSC-8
  escape-sequence hyperlinks plus hover preview (`render.rs:1703`).
- [ ] **Tab drag-to-reorder.** Missing. GPUI has drag preview, drop slots, and
  auto-scroll (`terminal_view/tab_strip/state.rs`, `tabs/drag.rs`).
- [ ] **Scrollbar markers** (search hits / line marks) — GPUI renders marker
  buckets in the scrollbar (`terminal_view/scrollbar.rs`); native scrollbar is
  thumb-only.
- [ ] **Ligatures / complex text layout / subpixel.** Native draws per-cell
  `Text(verbatim:)`; no shaping. Decide scope (most terminals omit ligatures,
  but document it).
- [ ] **Toasts / update progress UI** (`terminal_view/update_toasts.rs`,
  `overlay_view.rs`).

### M3 — tmux parity

- [ ] **tmux control mode.** Native only execs a shell into a per-pane
  `tmux` session (`Support/TmuxIntegration.swift`); it is not control-mode.
  GPUI implements full control-mode sync, snapshots, and native↔tmux
  transitions (`terminal_view/runtime/tmux/`). This is a large, distinct effort.

### M4 — Config, onboarding, store

- [ ] **Full config import.** Native imports only font family + size and only
  detects alacritty/kitty/ghostty (`Support/TerminalConfigImport.swift:37-96`).
  GPUI has complete importers for **alacritty, ghostty, iterm2, kitty** including
  colors/keybinds (`onboarding/import/`).
- [ ] **Settings UI depth.** GPUI has searchable sections, live preview, color
  picker, keybind editor, theme store with auth (`settings_view/`,
  `theme_store.rs`). Native is schema-driven straight-through edits with a
  keybind filter field only (`Views/Settings/SettingsRootView.swift:385`) — no
  full-settings search, no live preview, no color picker. Close these.
- [ ] **Theme store** (browse/install, ETag cache, auth) — `theme_store.rs`,
  deeplink `termy://store/theme-install`.
- [ ] **Onboarding flow** parity (theme recommendations, import prompts) —
  `onboarding/`.

### M5 — Platform & lifecycle

- [ ] **Deeplinks** (`termy://new|settings|open/config|store/...`) —
  `deeplink.rs`. Confirmed **absent** in native: no URL scheme is registered in
  the generated `Info.plist` (`script/build_and_run.sh`), no `onOpenURL`/
  `application(_:open:)` handler exists, and the only `termy://`-adjacent code is
  a comment. Needs scheme registration + a router.
- [ ] **CLI delegation + install.** GPUI dispatches non-GUI args to the CLI and
  installs the `termy` shim (`cli_delegate.rs`,
  `terminal_view/interaction/install_cli.rs`). README confirms native has no CLI
  launcher.
- [ ] **Thermal awareness** — GPUI observes thermal state to throttle
  (`macos_thermal_observer.rs`); fold into the P3 cadence work.
- [ ] **Auto-update.** Native surfaces GitHub release links only
  (`Support/AppUpdater.swift`); confirm that matches the intended update story
  (GPUI also link-based, so likely fine — just verify).
- [ ] **Native context menu** parity (`terminal_view/interaction/context_menu.rs`
  via `termy_native_sdk`).

---

## 3. Testing & quality gates

Native coverage is config/persistence/stress only — no rendering, input, or
interaction tests (`Tests/TermySwiftTests/`).

- [ ] **Render-correctness tests** (golden frames for partial vs full redraw)
  once P1/P2 land.
- [ ] **Input/encoding tests** (keyboard, mouse, IME marked-text) — encoding is
  in Rust, but the Swift event→FFI mapping is untested.
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
