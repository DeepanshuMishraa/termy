# Native macOS production roadmap

Status date: 2026-07-13

This roadmap takes the SwiftUI/AppKit host in `macos/` from a strong experimental
implementation to the default production macOS build of Termy. It is ordered:
each task has an explicit exit gate, and production code signing/notarization is
intentionally the final task.

## Current baseline

The native host is closer to production than the older roadmap suggests.

- Swift 6 warnings-as-errors builds successfully.
- 224 Swift tests pass; one native tab-bar test skips when AppKit does not expose
  the private tab-bar views in the test session.
- The Rust FFI test suite passes, including header/export contract tests.
- Native arm64 and x86_64 release DMGs build and their disk-image checksums verify.
- The arm64 release app passes the current launch gate: fast process startup,
  under 200 MiB RSS, and low sampled idle CPU.
- Native tmux control layout rendering and pane input are wired in the live
  SwiftUI workspace. The older roadmap still calling this GUI work unfinished is
  stale.
- The native Swift workflow is green at the current `main` head, including the
  unsigned arm64/x86_64 release matrix.

Known production blockers:

- The representative performance workflow has not completed successfully on a
  committed candidate. Its latest run exposed a short-lived latency-scenario
  process that can exit before `xctrace` attaches on a cold hosted runner; the
  lifecycle fix is locally verified and still needs a green remote run.
- The main release workflow still publishes the GPUI macOS app, not the native
  app.
- Interaction-heavy behavior still needs clean-machine and real-user testing:
  IME candidate placement, VoiceOver focus, selection/scrollback, native tabs,
  and tmux layouts.

## Definition of production ready

The native app is production ready only when all of the following are true:

1. A release bundle contains every runtime component and works without the
   repository, build directory, or developer shell environment.
2. Both supported architectures pass identical build, bundle, launch, and
   interaction gates.
3. Terminal rendering stays within measured budgets under idle, bulk-output,
   full-screen TUI, split-pane, resize, and scrollback workloads.
4. Core keyboard, mouse, selection, search, IME, accessibility, tabs, tmux,
   persistence, configuration, and update flows have automated coverage plus a
   documented manual test matrix.
5. Release CI produces deterministic native artifacts with checksums and a
   tested rollback path.
6. A release candidate completes a beta soak with no open P0/P1 issue.
7. The final externally distributed artifacts pass Apple trust validation and a
   clean-machine download-and-launch test.

## Delivery policy

- Keep the GPUI macOS build available until the native release candidate passes
  the full exit gate.
- Fix release and test-path drift before adding optional product features.
- Prefer measurable gates over roadmap checkboxes.
- Treat arm64 and x86_64 as equal release targets until support policy changes.
- Tasks 1–9 must remain runnable without production signing credentials.
- Task 10 is the only production signing/notarization task and is deliberately
  last.

## Task summary

| Task | Outcome | Estimated effort | Status |
| --- | --- | ---: | --- |
| 1 | Make roadmap and production status truthful | 0.5–1 day | Done 2026-07-10 |
| 2 | Produce a complete, self-contained native app bundle | 1–2 days | Done 2026-07-10 |
| 3 | Turn bundle readiness into a real unsigned release gate | 1–2 days | Done 2026-07-10 |
| 4 | Make rendering and launch performance gates representative | 2–3 days | CI lifecycle fix locally verified; green run pending |
| 5 | Close terminal interaction and tmux correctness gaps | 2–4 days | In progress; real AppKit/tmux/tab lifecycle smokes green locally |
| 6 | Complete accessibility, IME, lifecycle, and failure-path QA | 2–3 days | In progress; cursor-cell IME anchoring green locally |
| 7 | Wire native artifacts into release CI without cutover | 1–2 days | Candidate workflow implemented locally; dispatch evidence pending |
| 8 | Run clean-environment beta and soak testing | 3–7 elapsed days | Pending |
| 9 | Freeze and approve the unsigned production release candidate | 1 day | Pending |
| 10 | Code-sign, notarize, validate, and publish | 1–2 days | Pending |

The estimates assume one developer familiar with the repository. Beta soak time
is elapsed time rather than full-time engineering effort.

## Task 1 — Make the roadmap and status truthful ✅

Completed: 2026-07-10

### Objective

Remove stale claims so the remaining work is driven by the live app rather than
already-completed milestones.

### Completed work

- Reconciled `macos/roadmap.md` and
  `macos/docs/remaining-roadmap-plan.md` with the live implementation.
- Confirmed and documented tmux control-mode GUI layout, pane focus, keyboard,
  mouse, paste, search, split, and close wiring.
- Rewrote the stale fallback-only comment in
  `Support/TmuxIntegration.swift`.
- Set the README channel to `developer preview` and linked this checklist.
- Moved optional polish into a non-blocking backlog.

### Exit gate

- No open roadmap item describes code that already exists.
- Every production blocker maps to a task in this file.
- Optional polish is clearly separated from release-blocking work.

## Non-blocking backlog

- Structured keybinding editor.
- Broader command-palette coverage.
- Theme-registry caching and richer offline behavior.
- Richer updater progress UX.
- Optional GPU renderer, only if measured AppKit performance misses the release
  budgets.

## Task 2 — Produce a complete, self-contained native bundle ✅

Completed: 2026-07-10

### Objective

Make `Termy.app` work from a clean machine without repository-relative fallback
paths or missing helper binaries.

### Completed work

- `macos/scripts/build-dmg.sh` builds `termy_ffi`, `termy_cli`, and the Swift
  release product for the selected target.
- Release and `cargo macos build` staging both bundle an executable
  `Contents/MacOS/termy-cli` and use the same bundle-manifest gate.
- The manifest gate enforces executable presence, architecture parity, the FFI
  install name, exact `@rpath/libtermy_ffi.dylib` linkage, and the absence of
  build-directory rpaths.
- `check-release-readiness.sh --app PATH` delegates to the shared manifest gate,
  so omitting the release CLI is a hard failure.
- `check-cli-install-smoke.sh --app PATH` exercises the real installer core with
  an explicit bundled source and isolated home, PATH, and shell profile.
- The installed CLI runs `--version`, inspects configuration, and opens the
  native app with a working-directory argument before the smoke cleans it up.
- The native CI workflow stages a self-contained app, validates its manifest,
  and runs the isolated CLI smoke.

### Exit gate

For both arm64 and x86_64:

- `Contents/MacOS/Termy` exists and is executable.
- `Contents/MacOS/termy-cli` exists, is executable, and matches the app
  architecture.
- `Contents/Frameworks/libtermy_ffi.dylib` exists and matches the app
  architecture.
- `otool -L` reports `@rpath/libtermy_ffi.dylib` and no build-directory path.
- The isolated CLI-install smoke test passes.

Verified for arm64 and x86_64 release bundles and DMGs on 2026-07-10.

## Task 3 — Make unsigned release readiness a real gate ✅

Completed: 2026-07-10

### Objective

Make the pre-distribution verifier prove bundle correctness instead of merely
checking that a few files exist.

### Completed work

- Separated unsigned/ad-hoc candidate validation from Developer ID signing,
  Gatekeeper, and notarization, which remain Task 10.
- Extended `check-release-readiness.sh` to validate staged apps or DMGs,
  including bundle directories, executable modes, identifier/version/minimum-OS
  metadata, URL registration, icon/logo resources, CLI presence, every Mach-O
  architecture, FFI linkage/install name, load-path hygiene, and placeholder
  identifiers.
- Added DMG checksum verification, controlled read-only mounting, exact root app
  checks, and `/Applications` symlink validation.
- Added `TERMY_LAUNCH_PROBE_FILE`: the app reports readiness only after AppKit
  presents a visible window containing a ready terminal surface.
- Added a clean HOME/XDG config launch gate that checks the probe instead of
  accepting a PID alone.
- Ad-hoc-sign unsigned candidates inside-out so post-link edits and staged
  resources form a valid, launchable bundle without production credentials.
- Added corruption regressions for missing CLI, wrong architecture, absolute
  FFI linkage, missing logo, placeholder bundle ID, and malformed DMG contents.
- Added arm64 and x86_64 unsigned release-gate CI jobs for every native packaging
  change.

### Exit gate

- Deleting or corrupting any required bundle component makes the gate fail.
- A wrong architecture or absolute FFI path makes the gate fail.
- A DMG containing the wrong app or missing `/Applications` link makes the gate
  fail.
- Both architecture artifacts pass from a clean temporary home/config state.

Verified for mounted arm64 and x86_64 DMGs on 2026-07-10. Both presented a
usable 1280×820 native window from isolated state, and every deliberate
corruption failed with the expected diagnostic.

## Task 4 — Make performance gates representative

Implementation completed locally: 2026-07-10. Final completion still requires
a green, non-cancelled `macOS Performance Gates` run from the committed
candidate.

CI lifecycle remediation implemented locally: 2026-07-12.

### Objective

Prove sustained terminal performance rather than allowing a short benchmark to
pass on a handful of frames.

### Work

- Fix `macos/scripts/check-render-perf.sh` so the benchmark must produce a
  meaningful minimum sample count, initially at least 100 presented frames.
- Make zero/short samples a hard failure.
- Add deterministic scenarios for:
  - idle cursor blink;
  - continuous bulk output;
  - large scrollback navigation;
  - maximized full-screen TUI redraw;
  - rapid window resize;
  - 2, 4, and 8 split panes;
  - tmux control-mode pane output;
  - search with many matches.
- Record CPU render-plan p50/p95/p99, displayed-frame p50/p95/p99, full versus
  partial rebuild counts, patched-cell counts, skipped presents, and hitches.
- Add a windowed `xctrace` comparison against the current GPUI app for GPU/present
  timing.
- Replace the single post-launch CPU sample with a settled multi-sample window.
- Measure one-tab and multi-pane RSS separately.
- Calibrate budgets from stable baseline runs, then enforce them in CI. Initial
  release targets should include:
  - no accidental full rebuild loop during partial updates;
  - CPU render-plan p95 below the existing 2 ms ceiling;
  - no regression beyond the agreed GPUI threshold;
  - startup to usable window below the agreed release budget;
  - stable idle CPU after the initial launch work settles;
  - bounded memory after repeated tab/pane creation and closure.
- Make the performance workflow complete successfully on the release candidate;
  cancelled runs do not count as evidence.
- Retain benchmark reports as CI artifacts and compare trends over time.

### Exit gate

- Every scenario emits enough samples to be statistically meaningful.
- The current candidate has a green, non-cancelled performance workflow.
- The native app meets the agreed GPUI comparison thresholds on a real windowed
  run.
- A deliberate full-redraw or artificial delay regression makes CI fail.

### Local evidence

- All 10 deterministic native scenarios emitted at least 100 presented frames
  and render-plan samples. Their p95 render-plan build times remained below the
  2 ms ceiling.
- Forced-full-redraw and injected-5-ms-delay fixtures both failed the gate.
- Settled launch sampling measured a 313 ms usable one-tab window, 0.01% mean /
  0.10% p95 idle CPU, and 150.91 MiB max RSS. The verified eight-pane run
  reached a usable window in 800 ms, used 1.74% mean / 4.30% p95 CPU, and
  141.81 MiB max RSS.
- Attached, foreground window traces produced 571 GPUI versus 373 native
  steady-scroll frames at 16.67/16.67 ms p95, and 262 GPUI versus 247 native
  full-screen-TUI frames at 25.00/33.33 ms p95. All four traces reported zero
  hitches and passed the calibrated comparison gate.
- The performance workflow now builds the native candidate, runs deterministic
  and deliberate-regression gates, samples launch/resources, runs the windowed
  GPUI comparison, and retains JSON/Markdown reports for 30 days.
- Remote run
  [`29171354926`](https://github.com/lassejlv/termy/actions/runs/29171354926)
  proved that `idle-burst` could finish before cold hosted runners attached
  `xctrace`, failing both jobs with `Cannot find process for provided pid`.
- The latency drivers now remain alive for the complete requested duration.
  Three-second direct probes measured 3.27 seconds for `idle-burst` and 3.00
  seconds for `echo-train`; a local end-to-end `idle-burst` comparison completed
  all four Activity Monitor and Animation Hitches attach/export passes.

The remaining unchecked exit evidence is a green, non-cancelled workflow run
from a committed candidate containing the lifecycle fix; local tests cannot
establish that GitHub result.

## Task 5 — Close interaction and tmux correctness gaps

Started: 2026-07-12.

### Objective

Make the terminal reliable under real keyboard, mouse, selection, tab, and tmux
usage rather than only model-level tests.

### Progress

- Added a reusable `AppKitEventHarness` that mounts the real
  `KeyboardCaptureView` in an `NSWindow` and dispatches events through
  `NSWindow.sendEvent` and the AppKit first-responder path.
- Covered plain, control, command, option, shift, function, navigation, keypad,
  and repeated input plus full AppKit-event-to-libtermy legacy and Kitty
  key-encoding paths.
- Covered mouse press, drag, and release routing with modifiers; SGR mouse
  encoding through libtermy; and selection fallback when terminal mouse
  reporting is disabled.
- Covered line-wheel and precise trackpad-scroll accumulation plus search
  shortcut and Escape-dismiss precedence through AppKit events.
- Added real libtermy-backed search interaction tests for case sensitivity,
  regex, next/previous wrapping, scrollback matches, and output arriving while
  search remains open, plus resize while preserving the active match.
- Added AppKit-to-view-model selection tests for wide glyphs and changing
  output. They exposed and fixed phantom spaces when copying wide characters by
  carrying an explicit wide-character-spacer bit through core, the C ABI, and
  Swift instead of treating every non-rendered cell as a blank.
- Carried Alacritty's soft-wrap marker through the same core/C ABI/Swift path so
  copied wrapped commands no longer gain invented newlines. AppKit selection now
  covers soft wraps, a real scrollback viewport, and drag clamping at a pane
  boundary.
- Added an AppKit native-tab lifecycle test for ordering, rename, pin, reorder,
  selection, close, and title synchronization. Full-suite execution exposed and
  fixed reorder routing that incorrectly used the process key window instead of
  the dragged descriptor's own tab group. Pinned/manual titles already
  round-trip through workspace snapshot restore tests.
- Added a real tmux 3.7b workspace smoke with a private socket and deterministic
  cleanup. It drives the AppKit input view through nested horizontal/vertical
  layouts, focused-pane input, SGR mouse reporting, split/close, and search, and
  covers live resize propagation, sustained output into scrollback, copy,
  pane-process exit, control-session shutdown, and disabled/missing-binary
  decline paths.
- The focused AppKit event suite and the complete 212-test Swift suite are green
  locally. Wrapped/scrollback/pane-boundary selection and the deeper interaction
  scenarios below remain open.

### Remaining work

- Extend keyboard behavior coverage for:
  - live dead-key and alternate-layout sessions through the macOS input context;
  - Kitty keyboard protocol mode transitions beyond the covered report-all
    repeat path;
  - media-adjacent keys;
  - broader shortcut precedence versus terminal input.
- Extend mouse behavior coverage for:
  - move events;
  - terminal mouse modes beyond the covered SGR path;
  - split-divider dragging and scrollbar dragging.
- Extend native-tab coverage to the production create path and first/last-tab
  transitions; reorder, rename, pin, close, focus, title synchronization, and
  pinned/manual-title snapshot restore are covered.
- Exercise tmux control mode through the real GUI with:
  - a live control-mode startup failure followed by shell fallback.
- Keep the shell-backed fallback covered when tmux is absent or control mode
  fails.

### Exit gate

- The automated AppKit event suite covers the full input-to-FFI path.
- Selection, search, and scrollback have interaction coverage, not only clamp
  tests.
- Real tmux GUI smoke tests pass without pane desynchronization or misrouted
  input.
- No open P0/P1 interaction bug remains.

## Task 6 — Complete accessibility, IME, lifecycle, and failure-path QA

Started: 2026-07-13.

### Objective

Make the app safe for daily use across accessibility needs, international input,
process lifecycle changes, and damaged local state.

### Progress

- Routed the live terminal cursor position into `KeyboardCaptureView` and made
  `firstRect(forCharacterRange:)` return the cursor cell in screen coordinates
  instead of the entire terminal surface. The AppKit harness verifies the
  geometry remains correct after resize.
- Fixed a one-event IME commit path that cleared marked state before deciding
  whether to emit the committed text.
- Added UTF-8 commit coverage for representative Japanese, Simplified Chinese,
  Korean, and composed Latin text. These deterministic tests do not replace the
  live input-source and candidate-window matrix below.
- Added explicit updater response and semantic-version validation. Non-2xx
  GitHub responses, malformed versions, prerelease ordering, and non-GitHub
  manual-download URLs are covered.
- Added workspace persistence failure tests for missing, truncated/corrupt, and
  incompatible-version state plus failed writes. Restore/save failures already
  surface in the terminal UI; the banner now offers a scoped `Reset Workspace`
  action that deletes only the native workspace snapshot.
- Added real PTY lifecycle stress for normal and signal-terminated shell exits,
  plus 40 alternating split/create-close cycles that assert every removed pane
  releases its terminal view model.

### Work

- Test IME composition with at least Japanese, Simplified Chinese, Korean, and
  dead-key Latin input.
- Anchor the IME candidate window to the active cursor cell and verify it while
  scrolling, resizing, splitting, and switching tabs.
- Run a VoiceOver matrix covering:
  - focus entering the terminal grid;
  - visible text and current line;
  - cursor/insertion position;
  - selected text;
  - line navigation;
  - pane and native-tab changes;
  - settings and command palette.
- Verify keyboard-only operation for every core command.
- Stress lifecycle paths:
  - rapid native-tab create-close loops;
  - app quit with active terminals;
  - window close while output is arriving;
  - sleep/wake and display changes;
  - thermal throttling;
  - tmux/control-session teardown;
  - FFI wakeup thread teardown.
- Test configuration and persistence failures:
  - missing config;
  - malformed config with diagnostics;
  - read-only config directory;
  - truncated/corrupt workspace snapshot;
  - incompatible old snapshot;
  - full disk or failed write;
  - reset/recovery without losing unrelated config.
- Verify update checks for offline mode, GitHub errors, malformed versions,
  prereleases, and manual-download behavior.
- Add a user-visible diagnostics export containing versions, architecture,
  configuration diagnostics, and recent native logs without secrets.
- Run repeated open/close and long-output soak tests while checking for retained
  terminal sessions, wakeup threads, windows, and memory growth.

### Exit gate

- IME candidate placement is correct in live GUI testing.
- VoiceOver can focus and read the grid and selection without trapping keyboard
  input.
- Lifecycle stress completes without crashes, leaked sessions, or unbounded
  memory growth.
- Corrupt or unwritable local state produces a recoverable user-facing error.

## Task 7 — Add native artifacts to release CI without cutting over

Started: 2026-07-13. Workflow implementation is local; a clean hosted dispatch
is still required for exit evidence.

### Objective

Make CI produce and validate native release candidates while the GPUI app remains
the public fallback.

### Progress

- Added a separate `macOS Native Candidates` workflow triggered by manual
  dispatch or a published release. It is independent of the public `Release`
  workflow, so native failures cannot block or replace the GPUI artifacts.
- The arm64/x86_64 matrix verifies the requested version against the source
  version, builds unsigned native DMGs, reruns DMG readiness/usable-launch and
  isolated CLI-install gates, then uploads the exact DMG, SHA-256, and build
  metadata as unpublished 30-day Actions artifacts.
- Added `macos/docs/native-candidate-release.md` with the artifact contract and
  exact local reproduction commands.

### Work

- Add native arm64 and x86_64 build jobs to the release workflow or a dedicated
  reusable native-release workflow.
- Build native DMGs with `macos/scripts/build-dmg.sh` in unsigned candidate mode.
- Run the complete bundle-readiness and launch gates against each generated DMG.
- Upload both native DMGs as clearly named candidate artifacts, separate from the
  public GPUI artifacts.
- Generate SHA-256 checksums from the exact uploaded bytes.
- Record source commit, Rust toolchain, Swift/Xcode version, target triple, and
  bundle version in build metadata.
- Ensure release versions come from one authoritative value and match the app
  plist, file name, Git tag, and update-check comparison.
- Add artifact-retention long enough for beta and rollback analysis.
- Protect the GPUI release jobs from accidental removal until Task 9 approval.
- Add a documented local reproduction command for every CI packaging step.

### Exit gate

- A workflow dispatch produces both native candidate DMGs and checksums from a
  clean checkout.
- Both candidates pass structural, linkage, launch, and performance gates.
- The candidate artifacts remain unpublished to normal users.
- A failed native job cannot break the existing GPUI release path.

## Task 8 — Run beta and soak testing

### Objective

Find real-world interaction and lifecycle failures before changing the public
macOS release.

### Test matrix

- Apple Silicon on the minimum supported macOS version.
- Apple Silicon on the current macOS version.
- Intel on a supported macOS version.
- Fresh user account with no Termy configuration.
- Existing GPUI Termy user with realistic config, themes, tasks, keybinds, and
  persisted workspace data.
- Common shells: zsh, bash, fish.
- Common full-screen tools: tmux, vim/neovim, less, top/btop, fzf, and a TUI with
  mouse reporting.
- Multiple display scale factors and external display attach/detach.
- International input and VoiceOver sessions.

### Work

- Publish unsigned/internal candidate artifacts only to the test group.
- Run at least one multi-hour automated output/resize/tab cycle.
- Ask beta users to daily-drive the app for several days.
- Collect startup, CPU, memory, crash, input, and rendering reports with exact
  version and architecture.
- Triage every report into:
  - P0: data loss, security issue, or widespread launch failure;
  - P1: crash, unusable input, severe rendering corruption, or runaway resource
    use;
  - P2: meaningful workflow defect with a workaround;
  - P3: polish or optional parity.
- Fix all P0/P1 issues and rerun the relevant automated gate.
- Document accepted P2/P3 limitations in release notes.
- Verify that users can return to the GPUI build without losing config or
  workspace data.

### Exit gate

- Minimum three consecutive beta days with no new P0/P1 issue.
- No reproducible crash in the standard test matrix.
- No config/persistence incompatibility that prevents rollback.
- Resource measurements stay within the agreed budgets during soak.

## Task 9 — Freeze and approve the unsigned production candidate

### Objective

Create one immutable candidate that has passed every non-signing production gate
and is ready for final distribution processing.

### Work

- Freeze feature work; allow only release-blocking fixes.
- Select the release commit and version.
- Run from a clean checkout:
  - Rust FFI tests;
  - Swift warnings-as-errors build;
  - complete Swift tests;
  - config matrix;
  - native stress suite;
  - bundle-readiness checks for both architectures;
  - launch/resource gates;
  - representative render and GPUI comparison gates;
  - beta regression tests.
- Build both unsigned candidate DMGs from the frozen commit.
- Verify checksums and record the exact artifact manifest.
- Update README, roadmap, release notes, support instructions, known issues, and
  rollback instructions.
- Decide whether the release replaces GPUI immediately or ships as an explicit
  native beta channel. Default to beta unless the full test matrix is complete.
- Obtain human go/no-go approval for:
  - functionality;
  - performance;
  - compatibility;
  - rollback;
  - release notes;
  - artifact manifest.

### Exit gate

- The release commit is immutable and all non-signing checks are green.
- Both candidate artifacts match the recorded checksums and manifest.
- No P0/P1 issue is open.
- The rollback release and instructions are ready.
- Human go/no-go approval is recorded.

## Task 10 — Code-sign, notarize, validate, and publish

This is deliberately the final task. Do not begin it until Task 9 is complete.

### Objective

Turn the frozen, verified native candidates into trusted external artifacts and
publish them without changing their application contents afterward.

### Work

- Configure the correct Termy `Developer ID Application` identity and protected
  notarization credentials in the release environment.
- Keep credentials out of repository files, logs, artifacts, and forked pull
  request workflows.
- Confirm the minimum entitlements actually required by the app. Do not add
  speculative exceptions and do not ship `com.apple.security.get-task-allow`.
- For each architecture, sign inside-out with a secure timestamp and hardened
  runtime:
  1. `Contents/Frameworks/libtermy_ffi.dylib`;
  2. `Contents/MacOS/termy-cli`;
  3. `Contents/MacOS/Termy`;
  4. `Termy.app`;
  5. the final DMG.
- Verify before submission:
  - `codesign --verify --deep --strict --verbose=4 Termy.app`;
  - `codesign -dvvv --entitlements :- Termy.app`;
  - nested binaries carry the expected Developer ID team and hardened-runtime
    flags;
  - no development-only entitlement is present.
- Submit each final DMG with `xcrun notarytool submit --wait`.
- Review the complete notary log, including warnings.
- Staple and validate the ticket with `xcrun stapler`.
- Run Gatekeeper assessment on the app and DMG with `spctl`.
- Download the published bytes through the same public path users will use,
  confirm the checksum, and test the quarantined download on clean arm64 and
  Intel machines.
- On each clean machine verify:
  - normal Finder launch without bypass instructions;
  - first terminal startup and shell input;
  - native tabs and splits;
  - tmux mode;
  - settings/config load;
  - URL scheme;
  - “Install Command Line Tool…” and the installed CLI;
  - update check;
  - quit/relaunch and persistence.
- Publish the release only after both clean-machine tests pass.
- Monitor launch failures, crashes, update traffic, and support reports during
  the initial rollout; pause or roll back immediately for a P0/P1 regression.

### Exit gate

- Every nested executable and final artifact has a valid Developer ID signature,
  hardened runtime, and secure timestamp.
- Apple notarization succeeds with no unresolved warning and the ticket is
  stapled successfully.
- Gatekeeper accepts the quarantined public download on clean arm64 and Intel
  machines.
- Public checksums match the downloaded artifacts.
- The published release passes the full post-download smoke test and the GPUI
  rollback remains available.
