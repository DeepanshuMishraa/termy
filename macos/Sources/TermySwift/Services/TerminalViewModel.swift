import AppKit
import Darwin
import Foundation

@MainActor
final class TerminalViewModel: ObservableObject {
    @Published private(set) var frame: TerminalFrame = .empty
    @Published private(set) var errorMessage: String?
    @Published private(set) var renderConfig = TerminalRenderConfig.default
    @Published private(set) var title = "Shell"
    @Published private(set) var progress = TerminalProgress.clear
    @Published private(set) var isExited = false
    @Published private(set) var currentWorkingDirectory: String?
    @Published private(set) var searchMatches: [TerminalSearchMatch] = []
    @Published private(set) var activeSearchMatchIndex = 0
    @Published private(set) var hoveredLink: TerminalFrameLink?
    @Published private(set) var debugMetrics = TerminalDebugMetrics.empty
    @Published private(set) var nativeRenderMetrics = NativeRenderMetricsSnapshot()
    /// Bumped whenever the cached render plan changes, so the grid view redraws.
    /// The plan itself lives in `renderPlanCache` (not `@Published`) to avoid
    /// diffing large arrays on every frame.
    @Published private(set) var renderRevision = 0
    @Published private(set) var renderDamage = TerminalDamage.full
    /// Host-driven cursor blink visibility; the grid view skips the cursor
    /// while this is `false`. Ticked from the refresh driver in
    /// `pollAndPresent()` so blinking follows the active/idle cadence and
    /// pauses with suspended (occluded) tabs.
    @Published private(set) var cursorBlinkVisible = true
    @Published var selection: TerminalSelection?

    /// The flattened paint instructions for the current frame, rebuilt
    /// incrementally from damage in `pollAndPresent()`.
    var renderPlan: TerminalRenderPlan { renderPlanCache.plan }
    private let renderPlanCache = TerminalRenderPlanCache()
    private let frameStore = TerminalFrameStore()
    private let nativeRenderMetricsRecorder = NativeRenderMetricsRecorder()

    private var terminal: LibTermyTerminal?
    private var cursorBlinkPhase = TerminalCursorBlinkPhase()
    private var baseRenderConfig = TerminalRenderConfig.default
    private var fontSizeDelta: CGFloat = 0
    private var refreshDriver: DisplaySyncedRefreshDriver?
    private var cadence: RefreshCadence = .active
    private var lastActivityAt = Date()
    private static let idleCadenceThreshold: TimeInterval = 0.4
    private static let liveSearchRefreshInterval: TimeInterval = 0.15
    private var lastResize: TerminalResize?
    private var pendingResizeRefresh: Task<Void, Never>?
    private var lastResizeRefreshAt: Date?
    private var isSuspended = false
    private static let resizeRefreshInterval: TimeInterval = 1.0 / 60.0
    private var settingsObserver: NSObjectProtocol?
    private var appearanceObserver: NSObjectProtocol?
    private var startupRefreshUntil: Date?
    private let initialWorkingDirectory: String?
    private let startupCommand: String?
    private let tmuxSessionHint: String
    private var configuration = TermyConfigurationStore.shared.configuration
    private var activeSearchQuery = ""
    private var activeSearchOptions = TerminalSearchOptions()
    private var lastSearchRefreshAt: Date?
    private var lastAutoCopiedSelectionText: String?
    private var renderedFrameCount = 0
    private var skippedPresentCount = 0
    private var fullRebuildCount = 0
    private var partialRebuildCount = 0
    private var lastDebugSample = ProcessUsageSample.capture()

    init(
        workingDirectory: String? = nil,
        startupCommand: String? = nil,
        tmuxSessionHint: String = UUID().uuidString,
        restoredBufferText: String? = nil
    ) {
        initialWorkingDirectory = TerminalViewModel.normalizedWorkingDirectory(workingDirectory)
        self.startupCommand = TerminalViewModel.normalizedStartupCommand(startupCommand)
        self.tmuxSessionHint = tmuxSessionHint
        if let restoredBufferText = Self.normalizedRestoredBufferText(restoredBufferText) {
            let previewFrame = TerminalFrame.plainTextPreview(restoredBufferText)
            frameStore.reset(to: previewFrame)
            frame = previewFrame
            renderPlanCache.update(
                frame: previewFrame,
                renderConfig: renderConfig,
                damage: .full
            )
        }
    }

    func start() {
        guard terminal == nil else {
            return
        }

        do {
            // An explicit startup command (deeplink/task) wins; otherwise launch
            // inside tmux when the integration is enabled.
            let effectiveStartupCommand = startupCommand
                ?? TmuxIntegration.startupCommand(sessionHint: tmuxSessionHint)
            let terminal = try LibTermyTerminal(
                workingDirectoryOverride: initialWorkingDirectory,
                startupCommand: effectiveStartupCommand
            )
            self.terminal = terminal
            applyBaseRenderConfig(terminal.renderConfig)
            currentWorkingDirectory = initialWorkingDirectory
            isExited = false
            startupRefreshUntil = Date().addingTimeInterval(2)
            pollAndPresent(force: true)
            startRefreshDriver(.active)
            settingsObserver = NotificationCenter.default.addObserver(
                forName: .termySettingsChanged,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor in
                    self?.reloadConfiguration()
                }
            }
            appearanceObserver = DistributedNotificationCenter.default().addObserver(
                forName: Notification.Name("AppleInterfaceThemeChangedNotification"),
                object: nil,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor in
                    self?.reloadAppearance()
                }
            }
        } catch {
            report(error)
        }
    }

    /// Drives polling from display refresh boundaries: up to 60 Hz while the
    /// terminal is actively producing output or receiving input, backing off to
    /// 15 Hz once idle to save CPU/battery. Multiple core updates between ticks
    /// coalesce into one frame update before presentation.
    private func startRefreshDriver(_ cadence: RefreshCadence) {
        self.cadence = cadence
        if let refreshDriver {
            refreshDriver.cadence = cadence
            return
        }
        let driver = DisplaySyncedRefreshDriver { [weak self] in
            self?.pollAndPresent()
        }
        refreshDriver = driver
        driver.start(cadence: cadence)
    }

    private func noteActivity() {
        lastActivityAt = Date()
        if cadence != .active {
            startRefreshDriver(.active)
        }
    }

    private func adaptCadenceWhenIdle() {
        guard cadence == .active,
              Date().timeIntervalSince(lastActivityAt) > Self.idleCadenceThreshold
        else {
            return
        }
        startRefreshDriver(.idle)
    }

    /// Pauses refresh polling without tearing down the PTY, so the terminal core
    /// keeps running and buffers output while the hosting tab/window is occluded.
    /// This stops occluded background tabs from competing for the main run loop
    /// (e.g. while a divider is being dragged in the visible window).
    func suspendRefresh() {
        guard terminal != nil, !isSuspended else {
            return
        }
        isSuspended = true
        refreshDriver?.stop()
        refreshDriver = nil
        pendingResizeRefresh?.cancel()
        pendingResizeRefresh = nil
    }

    /// Resumes polling when the tab/window becomes visible again and forces one
    /// refresh to catch up on output that accumulated while suspended.
    func resumeRefresh() {
        guard terminal != nil, isSuspended else {
            return
        }
        isSuspended = false
        startRefreshDriver(.active)
        pollAndPresent(force: true)
    }

    func stop() {
        refreshDriver?.stop()
        refreshDriver = nil
        pendingResizeRefresh?.cancel()
        pendingResizeRefresh = nil
        isSuspended = false
        if let settingsObserver {
            NotificationCenter.default.removeObserver(settingsObserver)
            self.settingsObserver = nil
        }
        if let appearanceObserver {
            DistributedNotificationCenter.default().removeObserver(appearanceObserver)
            self.appearanceObserver = nil
        }
        terminal = nil
        isExited = true
        progress = .clear
        startupRefreshUntil = nil
    }

    /// Re-read appearance settings from the config file and apply them to this
    /// live terminal: refreshed render config (font/metrics/padding/opacity) and
    /// reloaded theme palette so existing cells recolor.
    private func reloadAppearance() {
        do {
            applyBaseRenderConfig(try LibTermyTerminal.loadRenderConfig())
            try terminal?.reloadColors()
            pollAndPresent(force: true)
        } catch {
            report(error)
        }
    }

    private func reloadConfiguration() {
        configuration = TermyConfigurationStore.shared.reload()
        if !configuration.native.progressIndicatorEnabled {
            progress = .clear
        }
        reloadAppearance()
    }

    func sendControlC() {
        send(bytes: [3])
    }

    func sendKey(_ keyInput: TerminalKeyInput) {
        do {
            guard let bytes = try terminal?.encodeKey(keyInput), !bytes.isEmpty else {
                return
            }
            send(bytes: bytes)
        } catch {
            report(error)
        }
    }

    func sendMouse(_ mouseInput: TerminalMouseInput) -> Bool {
        do {
            guard let bytes = try terminal?.encodeMouse(mouseInput), !bytes.isEmpty else {
                return false
            }
            send(bytes: bytes)
            return true
        } catch {
            report(error)
            return false
        }
    }

    func send(bytes: [UInt8]) {
        guard !bytes.isEmpty else {
            return
        }

        selection = nil
        if frame.displayOffset > 0 {
            scrollToBottom()
        }
        do {
            try terminal?.write(bytes)
            noteActivity()
            resetCursorBlinkPhase()
            pollAndPresent(force: true)
        } catch {
            report(error)
        }
    }

    /// Forces the cursor solid and restarts the blink interval, e.g. on input
    /// or when the pane gains focus.
    func resetCursorBlinkPhase() {
        cursorBlinkPhase.noteInput()
        if !cursorBlinkVisible {
            cursorBlinkVisible = true
        }
    }

    func scrollDisplay(deltaLines: Int) {
        guard deltaLines != 0 else {
            return
        }

        let clampedDelta = max(Int(Int32.min), min(Int(Int32.max), deltaLines))
        refreshIfChanged {
            try terminal?.scrollDisplay(deltaLines: Int32(clampedDelta)) == true
        }
    }

    func scrollToBottom() {
        refreshIfChanged {
            try terminal?.scrollToBottom() == true
        }
    }

    func scrollToDisplayOffset(_ offset: Int) {
        let targetOffset = max(0, min(offset, frame.historySize))
        scrollDisplay(deltaLines: targetOffset - frame.displayOffset)
    }

    func scrollToTop() {
        scrollToDisplayOffset(frame.historySize)
    }

    func clearScrollback() {
        refreshIfChanged {
            try terminal?.clearScrollback() == true
        }
    }

    func increaseFontSize() {
        adjustFontSize(by: 1)
    }

    func decreaseFontSize() {
        adjustFontSize(by: -1)
    }

    func resetFontSize() {
        guard fontSizeDelta != 0 else {
            return
        }
        fontSizeDelta = 0
        applyRenderConfig(baseRenderConfig, forceFullRebuild: true)
    }

    private func adjustFontSize(by delta: CGFloat) {
        let currentFontSize = baseRenderConfig.fontSize + fontSizeDelta
        let nextFontSize = Self.clampedFontSize(currentFontSize + delta)
        let nextDelta = nextFontSize - baseRenderConfig.fontSize
        guard nextDelta != fontSizeDelta else {
            return
        }
        fontSizeDelta = nextDelta
        applyRenderConfig(zoomedRenderConfig(from: baseRenderConfig), forceFullRebuild: true)
    }

    func updateSelection(_ selection: TerminalSelection?) {
        self.selection = selection
        guard renderConfig.copyOnSelect,
              let text = frame.selectedText(for: selection),
              !text.isEmpty
        else {
            lastAutoCopiedSelectionText = nil
            return
        }
        guard text != lastAutoCopiedSelectionText else {
            return
        }
        copy(text)
        lastAutoCopiedSelectionText = text
    }

    /// Double-click: select the word under the cursor.
    func selectWord(at position: TerminalGridPosition) {
        guard let selection = frame.wordSelection(at: position) else {
            updateSelection(nil)
            return
        }
        updateSelection(selection)
    }

    /// Triple-click: select the whole line under the cursor.
    func selectLine(at position: TerminalGridPosition) {
        updateSelection(frame.lineSelection(at: position))
    }

    func selectAll() {
        guard frame.cols > 0, frame.rows > 0 else {
            updateSelection(nil)
            return
        }
        updateSelection(TerminalSelection(
            anchor: TerminalGridPosition(col: 0, row: 0),
            active: TerminalGridPosition(col: frame.cols - 1, row: frame.rows - 1)
        ))
    }

    /// The link under `position`: an OSC 8 hyperlink reported by the core when
    /// present, otherwise heuristic text detection on the frame.
    private func link(at position: TerminalGridPosition) -> TerminalFrameLink? {
        if let link = terminal?.hyperlink(atRow: position.row, col: position.col) {
            return link
        }
        return frame.link(at: position)
    }

    /// Updates the link highlighted under the pointer. Returns true when a link
    /// is present so the view can show the pointing-hand cursor.
    @discardableResult
    func updateHoveredLink(at position: TerminalGridPosition?) -> Bool {
        let link = position.flatMap { self.link(at: $0) }
        if link != hoveredLink {
            hoveredLink = link
        }
        return link != nil
    }

    /// Opens the link under `position` (⌘-click). Returns true if one was opened.
    func openLink(at position: TerminalGridPosition) -> Bool {
        guard let link = link(at: position), let url = URL(string: link.target) else {
            return false
        }
        return NSWorkspace.shared.open(url)
    }

    func copySelection() -> Bool {
        guard let text = frame.selectedText(for: selection), !text.isEmpty else {
            return false
        }

        copy(text)
        return true
    }

    func search(
        _ query: String,
        options: TerminalSearchOptions = TerminalSearchOptions()
    ) -> [TerminalSearchMatch] {
        do {
            return try terminal?.search(query, options: options) ?? []
        } catch {
            report(error)
            return []
        }
    }

    func updateSearch(
        _ query: String,
        options: TerminalSearchOptions = TerminalSearchOptions()
    ) {
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else {
            activeSearchQuery = ""
            activeSearchOptions = options
            searchMatches = []
            activeSearchMatchIndex = 0
            lastSearchRefreshAt = nil
            return
        }

        let shouldResetActiveMatch = trimmedQuery != activeSearchQuery || options != activeSearchOptions
        activeSearchQuery = trimmedQuery
        activeSearchOptions = options
        refreshSearchMatches(resetActive: shouldResetActiveMatch, revealActive: true, force: true)
    }

    func selectNextSearchMatch() {
        guard !searchMatches.isEmpty else {
            return
        }
        activeSearchMatchIndex = (activeSearchMatchIndex + 1) % searchMatches.count
        revealActiveSearchMatch()
    }

    func selectPreviousSearchMatch() {
        guard !searchMatches.isEmpty else {
            return
        }
        activeSearchMatchIndex = (activeSearchMatchIndex + searchMatches.count - 1) % searchMatches.count
        revealActiveSearchMatch()
    }

    func resize(cols: Int, rows: Int, cellWidth: CGFloat, cellHeight: CGFloat) {
        let cols = max(2, min(cols, Int(UInt16.max)))
        let rows = max(2, min(rows, Int(UInt16.max)))
        let resize = TerminalResize(
            cols: UInt16(cols),
            rows: UInt16(rows),
            cellWidth: Float(cellWidth),
            cellHeight: Float(cellHeight)
        )
        guard resize != lastResize else {
            return
        }
        lastResize = resize

        do {
            // The FFI resize is cheap and keeps the PTY winsize correct, so run
            // it every step. The expensive forced refresh (full frame update +
            // plan rebuild + row renderer repaint) is throttled below so a
            // continuous drag — e.g. dragging a split divider, which crosses a
            // cell boundary every few pixels — does not block the main run loop
            // on every step.
            try terminal?.resize(
                cols: resize.cols,
                rows: resize.rows,
                cellWidth: resize.cellWidth,
                cellHeight: resize.cellHeight
            )
            guard !isSuspended else {
                return
            }
            // Resizing is user activity: keep the refresh driver at the active
            // cadence so reflowed output arriving mid-drag presents at 60 Hz.
            noteActivity()
            scheduleResizeRefresh()
        } catch {
            report(error)
        }
    }

    /// Coalesces the forced refresh during a continuous resize. Runs immediately
    /// when idle, then at most once per `resizeRefreshInterval`, always with a
    /// trailing refresh so the final drag size is rendered. `lastResize` already
    /// captured the latest dimensions synchronously, so the trailing snapshot
    /// reflects the end state.
    private func scheduleResizeRefresh() {
        let now = Date()
        if let last = lastResizeRefreshAt,
           now.timeIntervalSince(last) < Self.resizeRefreshInterval {
            guard pendingResizeRefresh == nil else {
                return
            }
            let delay = Self.resizeRefreshInterval - now.timeIntervalSince(last)
            pendingResizeRefresh = Task { @MainActor [weak self] in
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                guard let self, !Task.isCancelled else {
                    return
                }
                self.pendingResizeRefresh = nil
                self.lastResizeRefreshAt = Date()
                self.pollAndPresent(force: true)
            }
            return
        }
        lastResizeRefreshAt = now
        pollAndPresent(force: true)
    }

    private func pollAndPresent(force: Bool = false) {
        do {
            let events = try terminal?.drainEvents() ?? []
            handle(events)
            let hasEvents = !events.isEmpty
            let forceFull = force || shouldForceStartupRefresh()
            let update = try terminal?.frameUpdate(forceFull: forceFull)
            let applyResult = update.map(frameStore.apply)
                ?? TerminalFrameStoreApplyResult(
                    changed: false,
                    effectiveDamage: .none,
                    patchedCellCount: 0
                )
            if let update {
                nativeRenderMetricsRecorder.recordFrameUpdate(update, applyResult: applyResult)
            }
            let hasDamage = applyResult.effectiveDamage.hasChanges

            if hasEvents || hasDamage {
                noteActivity()
            } else {
                adaptCadenceWhenIdle()
            }

            let blinkToggled = cursorBlinkPhase.tick(blinkEnabled: renderConfig.cursorBlink)
            if blinkToggled {
                cursorBlinkVisible = cursorBlinkPhase.isVisible
            }

            guard forceFull || applyResult.changed else {
                presentCursorBlink(toggled: blinkToggled)
                updateDebugMetrics(renderedFrame: false)
                return
            }

            guard update != nil else {
                presentCursorBlink(toggled: blinkToggled)
                updateDebugMetrics(renderedFrame: false)
                return
            }

            frame = frameStore.frame
            errorMessage = nil
            let effectiveDamage = forceFull ? .full : applyResult.effectiveDamage
            // Fold the cursor cell into the published damage on a blink toggle
            // so the grid view repaints the cursor row alongside the frame's
            // own dirty rows. The plan cache below keeps the narrower
            // `effectiveDamage`: a blink changes no cell content.
            if blinkToggled, let cursor = frame.cursor {
                renderDamage = effectiveDamage.union(
                    TerminalDirtySpan(row: cursor.row, leftCol: cursor.col, rightCol: cursor.col)
                )
            } else {
                renderDamage = effectiveDamage
            }
            renderPlanCache.update(
                frame: frame,
                renderConfig: renderConfig,
                damage: effectiveDamage
            )
            nativeRenderMetricsRecorder.recordPresentedFrame(planStats: renderPlanCache.stats)
            nativeRenderMetrics = nativeRenderMetricsRecorder.snapshot
            renderRevision &+= 1
            updateDebugMetrics(renderedFrame: true)
            refreshSearchMatches(resetActive: false, revealActive: false, force: false)
        } catch {
            report(error)
        }
    }

    /// Publishes a cursor-cell-only damage span when a blink toggle happens on
    /// a tick that presented no new frame, so the grid view repaints just the
    /// cursor row instead of re-applying stale (possibly full) damage.
    private func presentCursorBlink(toggled: Bool) {
        guard toggled, let cursor = frame.cursor, frame.displayOffset == 0 else {
            return
        }
        renderDamage = .partial([
            TerminalDirtySpan(row: cursor.row, leftCol: cursor.col, rightCol: cursor.col)
        ])
    }

    private func shouldForceStartupRefresh() -> Bool {
        guard let startupRefreshUntil else {
            return false
        }
        if Date() < startupRefreshUntil {
            return true
        }
        self.startupRefreshUntil = nil
        return false
    }

    private func refreshIfChanged(_ operation: () throws -> Bool) {
        do {
            if try operation() {
                pollAndPresent(force: true)
            }
        } catch {
            report(error)
        }
    }

    private func report(_ error: Error) {
        errorMessage = String(describing: error)
    }

    private func applyBaseRenderConfig(_ config: TerminalRenderConfig) {
        baseRenderConfig = config
        fontSizeDelta = Self.clampedFontSize(config.fontSize + fontSizeDelta) - config.fontSize
        applyRenderConfig(zoomedRenderConfig(from: config), forceFullRebuild: true)
    }

    private func zoomedRenderConfig(from config: TerminalRenderConfig) -> TerminalRenderConfig {
        let nextFontSize = Self.clampedFontSize(config.fontSize + fontSizeDelta)
        guard nextFontSize != config.fontSize else {
            return config
        }

        var nextConfig = config
        let scale = nextFontSize / config.fontSize
        nextConfig.fontSize = nextFontSize
        nextConfig.measuredCellWidth = max(1, config.measuredCellWidth * scale)
        nextConfig.measuredCellHeight = max(1, nextFontSize * config.lineHeight)
        return nextConfig
    }

    private func applyRenderConfig(_ config: TerminalRenderConfig, forceFullRebuild: Bool) {
        guard config != renderConfig else {
            return
        }

        renderConfig = config
        guard forceFullRebuild else {
            return
        }

        renderDamage = .full
        renderPlanCache.update(
            frame: frame,
            renderConfig: config,
            damage: .full
        )
        renderRevision &+= 1
    }

    private static func clampedFontSize(_ fontSize: CGFloat) -> CGFloat {
        min(72, max(6, fontSize))
    }

    private func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    private func updateDebugMetrics(renderedFrame: Bool) {
        if renderedFrame {
            renderedFrameCount += 1
            if renderPlanCache.stats.wasFullRebuild {
                fullRebuildCount += 1
            } else {
                partialRebuildCount += 1
            }
        } else {
            skippedPresentCount += 1
            nativeRenderMetricsRecorder.recordSkippedPresent()
            nativeRenderMetrics = nativeRenderMetricsRecorder.snapshot
        }

        let nextSample = ProcessUsageSample.capture()
        let elapsed = nextSample.timestamp.timeIntervalSince(lastDebugSample.timestamp)
        guard elapsed >= 1 else {
            return
        }

        let fps = Double(renderedFrameCount) / elapsed
        let cpuDelta = max(0, nextSample.cpuTime - lastDebugSample.cpuTime)
        debugMetrics = TerminalDebugMetrics(
            framesPerSecond: fps,
            cpuPercent: min(999, (cpuDelta / elapsed) * 100),
            memoryMegabytes: nextSample.memoryMegabytes,
            skippedPresentsPerSecond: Double(skippedPresentCount) / elapsed,
            fullRebuildsPerSecond: Double(fullRebuildCount) / elapsed,
            partialRebuildsPerSecond: Double(partialRebuildCount) / elapsed
        )
        renderedFrameCount = 0
        skippedPresentCount = 0
        fullRebuildCount = 0
        partialRebuildCount = 0
        lastDebugSample = nextSample
    }

    private func refreshSearchMatches(resetActive: Bool, revealActive: Bool, force: Bool) {
        guard !activeSearchQuery.isEmpty else {
            searchMatches = []
            activeSearchMatchIndex = 0
            lastSearchRefreshAt = nil
            return
        }

        let now = Date()
        guard force || resetActive || revealActive || shouldRefreshLiveSearch(at: now) else {
            return
        }

        let matches = search(activeSearchQuery, options: activeSearchOptions)
        lastSearchRefreshAt = now
        searchMatches = matches
        if matches.isEmpty {
            activeSearchMatchIndex = 0
            return
        }

        activeSearchMatchIndex = resetActive ? 0 : min(activeSearchMatchIndex, matches.count - 1)
        if revealActive {
            revealActiveSearchMatch()
        }
    }

    private func shouldRefreshLiveSearch(at now: Date) -> Bool {
        guard let lastSearchRefreshAt else {
            return true
        }
        return now.timeIntervalSince(lastSearchRefreshAt) >= Self.liveSearchRefreshInterval
    }

    private func revealActiveSearchMatch() {
        guard searchMatches.indices.contains(activeSearchMatchIndex), frame.rows > 0 else {
            return
        }

        let match = searchMatches[activeSearchMatchIndex]
        let visibleTop = frame.historySize - frame.displayOffset
        let visibleBottom = visibleTop + frame.rows - 1
        let targetOffset: Int
        if match.row < visibleTop {
            targetOffset = frame.historySize - match.row
        } else if match.row > visibleBottom {
            targetOffset = frame.historySize - (match.row - frame.rows + 1)
        } else {
            return
        }

        let clampedOffset = max(0, min(frame.historySize, targetOffset))
        scrollDisplay(deltaLines: clampedOffset - frame.displayOffset)
    }

    private func handle(_ events: [TerminalRuntimeEvent]) {
        guard !events.isEmpty else {
            return
        }

        for event in events {
            switch event {
            case .title(let title):
                if !title.isEmpty {
                    self.title = title
                }
            case .resetTitle:
                title = "Shell"
            case .exit:
                isExited = true
                progress = .clear
            case .progress(let progress):
                if configuration.native.progressIndicatorEnabled {
                    self.progress = progress
                }
            case .workingDirectory(let path):
                currentWorkingDirectory = TerminalViewModel.normalizedWorkingDirectory(path)
            case .clipboardStore(let text):
                // OSC 52: an app (tmux, vim, ssh) asked to set the system
                // clipboard. The Rust side already base64-decodes the payload.
                if !text.isEmpty {
                    copy(text)
                }
            case .wakeup,
                 .bell,
                 .shellPromptStart,
                 .shellCommandStart,
                 .shellCommandExecuting,
                 .shellCommandFinished(_):
                guard configuration.native.shellIntegrationEnabled else {
                    break
                }
                break
            }
        }
    }

    private static func normalizedWorkingDirectory(_ value: String?) -> String? {
        guard let value else {
            return nil
        }

        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty || trimmed.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains) {
            return nil
        }

        return trimmed
    }

    private static func normalizedStartupCommand(_ value: String?) -> String? {
        guard let value else {
            return nil
        }

        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private static func normalizedRestoredBufferText(_ value: String?) -> String? {
        guard let value else {
            return nil
        }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    func visibleTextSnapshot() -> String {
        frame.visibleTextSnapshot()
    }

}

struct TerminalDebugMetrics: Equatable {
    var framesPerSecond: Double
    var cpuPercent: Double
    var memoryMegabytes: Double
    /// Display-synced polls in the last sample window that had no frame to present.
    var skippedPresentsPerSecond: Double
    /// Frames in the last sample window that rebuilt the entire render plan.
    var fullRebuildsPerSecond: Double
    /// Frames in the last sample window that rebuilt only the damaged rows.
    var partialRebuildsPerSecond: Double

    static let empty = TerminalDebugMetrics(
        framesPerSecond: 0,
        cpuPercent: 0,
        memoryMegabytes: 0,
        skippedPresentsPerSecond: 0,
        fullRebuildsPerSecond: 0,
        partialRebuildsPerSecond: 0
    )
}

private struct ProcessUsageSample {
    var timestamp: Date
    var cpuTime: TimeInterval
    var memoryMegabytes: Double

    static func capture() -> ProcessUsageSample {
        var usage = rusage()
        _ = getrusage(RUSAGE_SELF, &usage)
        let userCPU = TimeInterval(usage.ru_utime.tv_sec)
            + TimeInterval(usage.ru_utime.tv_usec) / 1_000_000
        let systemCPU = TimeInterval(usage.ru_stime.tv_sec)
            + TimeInterval(usage.ru_stime.tv_usec) / 1_000_000

        return ProcessUsageSample(
            timestamp: Date(),
            cpuTime: userCPU + systemCPU,
            memoryMegabytes: Double(max(0, usage.ru_maxrss)) / 1_048_576
        )
    }
}

private struct TerminalResize: Equatable {
    var cols: UInt16
    var rows: UInt16
    var cellWidth: Float
    var cellHeight: Float
}

enum RefreshCadence {
    case active
    case idle

    var interval: TimeInterval {
        switch self {
        case .active:
            return 1.0 / 60.0
        case .idle:
            return 1.0 / 15.0
        }
    }
}
