# Native macOS production roadmap

Status date: 2026-07-10

This roadmap takes the SwiftUI/AppKit host in `macos/` from a strong experimental
implementation to the default production macOS build of Termy. It is ordered:
each task has an explicit exit gate, and production code signing/notarization is
intentionally the final task.

## Current baseline

The native host is closer to production than the older roadmap suggests.

- Swift 6 warnings-as-errors builds successfully.
- 188 Swift tests pass; one AppKit tab-bar test skips outside a full GUI session.
- The Rust FFI test suite passes, including header/export contract tests.
- Native arm64 and x86_64 release DMGs build and their disk-image checksums verify.
- The arm64 release app passes the current launch gate: fast process startup,
  under 200 MiB RSS, and low sampled idle CPU.
- Native tmux control layout rendering and pane input are wired in the live
  SwiftUI workspace. The older roadmap still calling this GUI work unfinished is
  stale.
- The current native CI workflow is green.

Known production blockers:

- The native release DMG does not bundle `termy-cli`, although the app exposes
  “Install Command Line Tool…”.
- The bundle-readiness check validates structure but can pass an artifact that
  is not suitable for external distribution.
- The native render benchmark can pass with only a few presented frames and
  therefore does not yet prove sustained rendering performance.
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

| Task | Outcome | Estimated effort | Blocking |
| --- | --- | ---: | --- |
| 1 | Make roadmap and production status truthful | 0.5–1 day | Yes |
| 2 | Produce a complete, self-contained native app bundle | 1–2 days | Yes |
| 3 | Turn bundle readiness into a real unsigned release gate | 1–2 days | Yes |
| 4 | Make rendering and launch performance gates representative | 2–3 days | Yes |
| 5 | Close terminal interaction and tmux correctness gaps | 2–4 days | Yes |
| 6 | Complete accessibility, IME, lifecycle, and failure-path QA | 2–3 days | Yes |
| 7 | Wire native artifacts into release CI without cutover | 1–2 days | Yes |
| 8 | Run clean-environment beta and soak testing | 3–7 elapsed days | Yes |
| 9 | Freeze and approve the unsigned production release candidate | 1 day | Yes |
| 10 | Code-sign, notarize, validate, and publish | 1–2 days | Yes |

The estimates assume one developer familiar with the repository. Beta soak time
is elapsed time rather than full-time engineering effort.

## Task 1 — Make the roadmap and status truthful

### Objective

Remove stale claims so the remaining work is driven by the live app rather than
already-completed milestones.

### Work

- Reconcile `macos/roadmap.md` and `macos/docs/remaining-roadmap-plan.md` with
  the current implementation.
- Mark tmux control-mode GUI layout, pane focus, keyboard input, mouse input,
  search, split, and close wiring as implemented where the live source confirms
  it.
- Remove or rewrite the stale fallback-only comment in
  `Support/TmuxIntegration.swift`.
- Replace the blanket “highly experimental” README status with an explicit
  channel such as `developer preview`, `beta`, or `production candidate`.
- Move optional polish into a non-blocking backlog:
  - structured keybinding editor;
  - command-palette breadth;
  - theme-registry caching;
  - richer updater UX;
  - optional GPU renderer if measured performance requires it.
- Keep this file as the release checklist and link it from `macos/README.md`.

### Exit gate

- No open roadmap item describes code that already exists.
- Every production blocker maps to a task in this file.
- Optional polish is clearly separated from release-blocking work.

## Task 2 — Produce a complete, self-contained native bundle

### Objective

Make `Termy.app` work from a clean machine without repository-relative fallback
paths or missing helper binaries.

### Work

- Update `macos/scripts/build-dmg.sh` to build `termy_ffi`, `termy_cli`, and the
  Swift release product for the selected target.
- Copy the target-specific `termy-cli` into `Contents/MacOS/termy-cli` and make
  it executable.
- Keep the release bundle contract aligned with `cargo macos`, which already
  bundles the CLI for developer builds.
- Add one shared bundle-manifest check instead of letting the development and
  release staging paths drift independently.
- Verify that the app binary loads only the bundled
  `@rpath/libtermy_ffi.dylib`, with no absolute `target/` or developer-machine
  paths.
- Verify that the app, FFI library, and CLI all match the requested architecture.
- Test “Install Command Line Tool…” with an isolated temporary `HOME`, PATH, and
  shell profile so the test never modifies the developer’s real environment.
- Run the installed CLI from that isolated environment and verify at least:
  - `termy-cli --version`;
  - config inspection;
  - opening the native app with a working-directory argument.
- Add a regression test that fails if the release bundle omits the CLI.

### Exit gate

For both arm64 and x86_64:

- `Contents/MacOS/Termy` exists and is executable.
- `Contents/MacOS/termy-cli` exists, is executable, and matches the app
  architecture.
- `Contents/Frameworks/libtermy_ffi.dylib` exists and matches the app
  architecture.
- `otool -L` reports `@rpath/libtermy_ffi.dylib` and no build-directory path.
- The isolated CLI-install smoke test passes.

## Task 3 — Make unsigned release readiness a real gate

### Objective

Make the pre-distribution verifier prove bundle correctness instead of merely
checking that a few files exist.

### Work

- Split structural validation from final distribution trust validation. Tasks
  1–9 operate on unsigned release candidates; Task 10 owns external trust.
- Extend `macos/scripts/check-release-readiness.sh --app PATH` to verify:
  - required bundle directories and files;
  - executable modes;
  - bundle identifier parity;
  - semantic version and build version;
  - minimum macOS version;
  - URL-scheme registration;
  - icon and selectable logo resources;
  - the CLI helper;
  - architecture parity across all Mach-O files;
  - FFI install name and app rpath;
  - absence of repository/build-machine paths;
  - absence of placeholder bundle identifiers;
  - DMG integrity and expected root contents.
- Mount the generated DMG read-only in CI and validate the app copied from the
  mounted image, not only the staging directory.
- Launch the mounted release app and ensure the process reaches a usable window,
  not merely a visible PID.
- Add a temporary config/home directory so launch tests are deterministic and do
  not consume developer state.
- Ensure failures print the exact missing file, invalid field, architecture, or
  linkage path.
- Run this gate for both architectures on every packaging change.

### Exit gate

- Deleting or corrupting any required bundle component makes the gate fail.
- A wrong architecture or absolute FFI path makes the gate fail.
- A DMG containing the wrong app or missing `/Applications` link makes the gate
  fail.
- Both architecture artifacts pass from a clean temporary home/config state.

## Task 4 — Make performance gates representative

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

## Task 5 — Close interaction and tmux correctness gaps

### Objective

Make the terminal reliable under real keyboard, mouse, selection, tab, and tmux
usage rather than only model-level tests.

### Work

- Add an AppKit event harness for live `NSEvent` paths.
- Cover keyboard behavior for:
  - plain text and composed text;
  - control, option/meta, command, and shift combinations;
  - Kitty keyboard protocol modes;
  - dead keys and alternate keyboard layouts;
  - function, navigation, keypad, and media-adjacent keys;
  - shortcut precedence versus terminal input.
- Cover mouse behavior for:
  - press, release, move, drag, wheel, and high-resolution trackpad scroll;
  - terminal mouse modes and modifier encoding;
  - selection versus application mouse reporting;
  - split-divider dragging and scrollbar dragging.
- Add interaction tests for selection across wrapped lines, wide glyphs,
  scrollback boundaries, pane boundaries, and changing output.
- Add search tests for regex, case sensitivity, next/previous wrapping,
  scrollback matches, resize, and output arriving while search is open.
- Add multi-window/native-tab tests for create, reorder, rename, pin, close,
  restore, focus, title synchronization, and first/last-tab transitions.
- Exercise tmux control mode through the real GUI with:
  - nested horizontal/vertical layouts;
  - focus and input routing;
  - mouse reporting;
  - split/close/resize;
  - high output volume;
  - pane exit and session shutdown;
  - search and copy;
  - fallback behavior when tmux is absent or control mode fails.
- Remove stale fallback-only tmux comments after the GUI test passes.

### Exit gate

- The automated AppKit event suite covers the full input-to-FFI path.
- Selection, search, and scrollback have interaction coverage, not only clamp
  tests.
- Real tmux GUI smoke tests pass without pane desynchronization or misrouted
  input.
- No open P0/P1 interaction bug remains.

## Task 6 — Complete accessibility, IME, lifecycle, and failure-path QA

### Objective

Make the app safe for daily use across accessibility needs, international input,
process lifecycle changes, and damaged local state.

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
  - shell exits normally and abnormally;
  - rapid tab/pane create-close loops;
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

### Objective

Make CI produce and validate native release candidates while the GPUI app remains
the public fallback.

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
