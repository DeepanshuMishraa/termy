import AppKit
import SwiftUI

struct TerminalSurfaceView: View {
    @ObservedObject var terminal: TerminalViewModel
    @State private var isScrollBarVisible = false
    @State private var scrollBarHideTask: Task<Void, Never>?
    @State private var lastSize: CGSize = .zero
    @State private var bellFlashOpacity: Double = 0

    let isFocused: Bool
    let showsFocusBorder: Bool
    let isInputEnabled: Bool
    let isSearchVisible: Bool
    let windowTitle: String
    let onFocus: () -> Void
    let onSplitRight: () -> Void
    let onSplitDown: () -> Void
    let onClosePane: () -> Void
    let onClosePaneIfSplit: () -> Bool
    let onFocusNextPane: () -> Void
    let onShowSearch: () -> Void
    let onDismissSearch: () -> Void
    var sendBytesOverride: (([UInt8]) -> Void)?
    var sendKeyOverride: ((TerminalKeyInput) -> Void)?
    var sendMouseOverride: ((TerminalMouseInput) -> Bool)?
    var pasteOverride: ((String) -> Void)?

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                TerminalGridView(
                    frame: terminal.frame,
                    renderPlan: terminal.renderPlan,
                    renderDamage: terminal.renderDamage,
                    selection: terminal.selection,
                    renderConfig: terminal.renderConfig,
                    searchMatches: terminal.searchMatches,
                    activeSearchMatch: terminal.searchMatches[safe: terminal.activeSearchMatchIndex],
                    hoveredLink: terminal.hoveredLink,
                    isFocused: isFocused,
                    isCursorVisible: terminal.cursorBlinkVisible
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .opacity(paneContentOpacity)
                .onChange(of: isFocused) { _, nowFocused in
                    terminal.setPaneFocused(nowFocused)
                }
                .onAppear {
                    terminal.setPaneFocused(isFocused)
                }

                TerminalKeyboardInputView(
                    cols: terminal.frameMetadata.cols,
                    rows: terminal.frameMetadata.rows,
                    cursorPosition: terminal.frameMetadata.cursor.map {
                        TerminalGridPosition(col: $0.col, row: $0.row)
                    },
                    renderConfig: terminal.renderConfig,
                    isFocused: isFocused,
                    isInputEnabled: isInputEnabled,
                    isSearchVisible: isSearchVisible,
                    canCopy: terminal.canCopySelection,
                    onFocus: onFocus,
                    onBytes: { bytes in
                        sendBytesOverride?(bytes) ?? terminal.send(bytes: bytes)
                    },
                    onKeyInput: { keyInput in
                        sendKeyOverride?(keyInput) ?? terminal.sendKey(keyInput)
                    },
                    onMouseInput: { mouseInput in
                        sendMouseOverride?(mouseInput) ?? terminal.sendMouse(mouseInput)
                    },
                    onScrollLines: { lines in
                        revealScrollBar()
                        terminal.scrollDisplay(deltaLines: lines)
                    },
                    onScrollToTop: {
                        revealScrollBar()
                        terminal.scrollToTop()
                    },
                    onScrollToBottom: {
                        revealScrollBar()
                        terminal.scrollToBottom()
                    },
                    onClearBuffer: {
                        terminal.clearScrollback()
                    },
                    onSplitRight: onSplitRight,
                    onSplitDown: onSplitDown,
                    onClosePane: onClosePane,
                    onClosePaneIfSplit: onClosePaneIfSplit,
                    onFocusNextPane: onFocusNextPane,
                    onShowSearch: onShowSearch,
                    onDismissSearch: onDismissSearch,
                    onSelectionChanged: { selection in
                        terminal.updateSelection(selection)
                    },
                    onSelectWord: { position in
                        terminal.selectWord(at: position)
                    },
                    onSelectLine: { position in
                        terminal.selectLine(at: position)
                    },
                    onSelectAll: {
                        terminal.selectAll()
                    },
                    onHoverProbe: { position in
                        terminal.updateHoveredLink(at: position)
                    },
                    onOpenLink: { position in
                        terminal.openLink(at: position)
                    },
                    onCopy: {
                        terminal.copySelection()
                    },
                    onPaste: { text in
                        pasteOverride?(text) ?? terminal.paste(text)
                    },
                    onMarkedTextChanged: { terminal.setMarkedText($0) }
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)

                markedTextOverlay

                TerminalTopLoader(progress: terminal.progress)
                    .frame(maxWidth: .infinity, alignment: .topLeading)

                if let errorMessage = terminal.errorMessage {
                    Text(errorMessage)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(.red)
                        .padding(8)
                        .background(.regularMaterial)
                        .padding(8)
                }

                if terminal.isExited && !terminal.hasVisibleContent {
                    Text("Process exited")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .padding(8)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                }
            }
            .overlay {
                focusEffectOverlay
            }
            .overlay(alignment: .trailing) {
                if shouldShowScrollBar {
                    TerminalScrollBar(
                        frameMetadata: terminal.frameMetadata,
                        renderConfig: terminal.renderConfig,
                        searchMatches: terminal.searchMatches,
                        commandMarks: terminal.commandMarkRows,
                        onInteraction: revealScrollBar,
                        onScrollToOffset: { offset in
                            revealScrollBar()
                            terminal.scrollToDisplayOffset(offset)
                        }
                    )
                    .frame(width: 14)
                    .padding(.trailing, 3)
                    .transition(.opacity)
                }
            }
            .overlay {
                if bellFlashOpacity > 0 {
                    Rectangle()
                        .fill(terminal.renderConfig.foreground.swiftUIColor)
                        .opacity(bellFlashOpacity)
                        .allowsHitTesting(false)
                }
            }
            .onChange(of: terminal.bellPulse) { _, _ in
                bellFlashOpacity = 0.2
                withAnimation(.easeOut(duration: 0.3)) {
                    bellFlashOpacity = 0
                }
            }
            .background(
                terminal.renderConfig.background.swiftUIColor
                    .opacity(terminal.renderConfig.backgroundOpacity)
            )
            .background(TerminalWindowChromeSyncView(
                title: windowTitle,
                isFocused: isFocused,
                background: terminal.renderConfig.background,
                backgroundOpacity: terminal.renderConfig.backgroundOpacity,
                backgroundBlur: terminal.renderConfig.backgroundBlur
            ))
            .background(NativeLaunchProbeView(terminalReady: terminal.isReady))
            .onTapGesture {
                onFocus()
                if isSearchVisible {
                    onDismissSearch()
                }
            }
            .onAppear {
                onFocus()
                terminal.start()
                lastSize = proxy.size
                resizeTerminal(to: proxy.size)
            }
            .onChange(of: proxy.size) { _, size in
                lastSize = size
                resizeTerminal(to: size)
            }
            .onChange(of: terminal.renderConfig) { _, _ in
                // Font/padding changes alter cell metrics, so recompute the grid
                // for the current pixel size when settings live-apply.
                resizeTerminal(to: lastSize)
            }
            .onChange(of: terminal.frameMetadata.displayOffset) { oldValue, newValue in
                if oldValue != newValue {
                    revealScrollBar()
                }
            }
        }
    }

    private func resizeTerminal(to size: CGSize) {
        let availableWidth = size.width - (terminal.renderConfig.paddingX * 2)
        let availableHeight = size.height - (terminal.renderConfig.paddingY * 2)
        terminal.resize(
            cols: terminalGridCount(availableWidth, cellSize: terminal.renderConfig.cellWidth),
            rows: terminalGridCount(availableHeight, cellSize: terminal.renderConfig.cellHeight),
            cellWidth: terminal.renderConfig.cellWidth,
            cellHeight: terminal.renderConfig.cellHeight
        )
    }

    private func terminalGridCount(_ availableSize: CGFloat, cellSize: CGFloat) -> Int {
        guard availableSize.isFinite, cellSize.isFinite, cellSize > 0 else {
            return 2
        }
        return max(2, Int(floor(max(0, availableSize) / cellSize)))
    }

    private func revealScrollBar() {
        guard terminal.frameMetadata.historySize > 0,
              terminal.renderConfig.scrollbarVisibility != .off
        else {
            return
        }

        isScrollBarVisible = true
        guard terminal.renderConfig.scrollbarVisibility == .onScroll else {
            return
        }
        scrollBarHideTask?.cancel()
        scrollBarHideTask = Task {
            try? await Task.sleep(nanoseconds: 1_200_000_000)
            guard !Task.isCancelled else {
                return
            }
            await MainActor.run {
                isScrollBarVisible = false
            }
        }
    }

    private var shouldShowScrollBar: Bool {
        guard terminal.frameMetadata.historySize > 0 else {
            return false
        }
        switch terminal.renderConfig.scrollbarVisibility {
        case .off:
            return false
        case .always:
            return true
        case .onScroll:
            return isScrollBarVisible
        }
    }

    private var focusStrength: Double {
        Double(terminal.renderConfig.paneFocusStrength)
    }

    private var paneContentOpacity: Double {
        Self.paneContentOpacity(
            showsFocusBorder: showsFocusBorder,
            isFocused: isFocused,
            effect: terminal.renderConfig.paneFocusEffect,
            strength: focusStrength
        )
    }

    nonisolated static func paneContentOpacity(
        showsFocusBorder: Bool,
        isFocused: Bool,
        effect: TerminalPaneFocusEffect,
        strength: Double
    ) -> Double {
        guard showsFocusBorder, !isFocused else {
            return 1.0
        }

        switch effect {
        case .off:
            return 1.0
        case .minimal:
            return max(0.82, 1.0 - (0.12 * strength))
        case .softSpotlight:
            return max(0.72, 1.0 - (0.34 * strength))
        case .cinematic:
            return max(0.62, 1.0 - (0.46 * strength))
        }
    }

    /// Draws the in-progress IME composition inline at the cursor cell, so CJK /
    /// dead-key input is visible in the grid before it commits.
    @ViewBuilder
    private var markedTextOverlay: some View {
        if !terminal.markedText.isEmpty, let cursor = terminal.frameMetadata.cursor {
            let config = terminal.renderConfig
            Text(terminal.markedText)
                .font(.custom(config.fontFamily, size: config.fontSize))
                .foregroundStyle(config.foreground.swiftUIColor)
                .background(config.background.swiftUIColor)
                .underline()
                .fixedSize()
                .offset(
                    x: config.paddingX + CGFloat(cursor.col) * config.cellWidth,
                    y: config.paddingY + CGFloat(cursor.row) * config.cellHeight
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .allowsHitTesting(false)
        }
    }

    @ViewBuilder
    private var focusEffectOverlay: some View {
        if showsFocusBorder {
            switch terminal.renderConfig.paneFocusEffect {
            case .off:
                EmptyView()
            case .minimal:
                if isFocused {
                    Rectangle()
                        .stroke(Color.accentColor.opacity(0.35 + (0.15 * focusStrength)), lineWidth: 1)
                        .allowsHitTesting(false)
                }
            case .softSpotlight:
                if isFocused {
                    Rectangle()
                        .stroke(Color.accentColor.opacity(0.45 + (0.12 * focusStrength)), lineWidth: 1)
                        .allowsHitTesting(false)
                } else {
                    Rectangle()
                        .fill(Color.black.opacity(0.08 * focusStrength))
                        .allowsHitTesting(false)
                }
            case .cinematic:
                if isFocused {
                    Rectangle()
                        .stroke(Color.accentColor.opacity(0.55 + (0.12 * focusStrength)), lineWidth: 1.25)
                        .shadow(color: Color.accentColor.opacity(0.16 * focusStrength), radius: 5)
                        .allowsHitTesting(false)
                } else {
                    Rectangle()
                        .fill(Color.black.opacity(0.14 * focusStrength))
                        .allowsHitTesting(false)
                }
            }
        }
    }
}

private struct TerminalWindowChromeSyncView: NSViewRepresentable {
    let title: String
    let isFocused: Bool
    let background: TerminalRGBA
    let backgroundOpacity: Double
    let backgroundBlur: Bool

    func makeNSView(context: Context) -> TerminalWindowChromeSyncNSView {
        let view = TerminalWindowChromeSyncNSView(frame: .zero)
        view.syncChrome(chromeState)
        return view
    }

    func updateNSView(_ view: TerminalWindowChromeSyncNSView, context: Context) {
        view.syncChrome(chromeState)
    }

    private var chromeState: TerminalWindowChromeState {
        let nextTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        return TerminalWindowChromeState(
            title: nextTitle.isEmpty ? "Shell" : nextTitle,
            isFocused: isFocused,
            background: background,
            backgroundOpacity: backgroundOpacity,
            backgroundBlur: backgroundBlur
        )
    }
}

struct TerminalWindowChromeState: Equatable {
    var title: String
    var isFocused: Bool
    var background: TerminalRGBA
    var backgroundOpacity: Double
    var backgroundBlur: Bool

    var effectiveBackgroundAlpha: CGFloat {
        CGFloat(min(1.0, max(0.0, background.alpha * backgroundOpacity)))
    }

    var requiresTransparentWindow: Bool {
        backgroundBlur || effectiveBackgroundAlpha < 0.999
    }

    var chromeBackgroundColor: NSColor {
        // Translucent surface: the SwiftUI background paints the tint once; a
        // colored window backing would tint the desktop show-through a second
        // time (compositing to 1-(1-a)^2 instead of a).
        if effectiveBackgroundAlpha < 0.999 {
            return .clear
        }
        // Opaque surface: AppKit composites the transparent titlebar over the
        // window backing, so keep it matching the terminal color so a hidden
        // tab bar blends with the terminal instead of the desktop.
        return background.nsColor.withAlphaComponent(CGFloat(background.alpha))
    }
}

@MainActor
enum TerminalWindowChromeApplier {
    @discardableResult
    static func apply(
        _ state: TerminalWindowChromeState,
        to window: NSWindow,
        appliedState: inout TerminalWindowChromeState?
    ) -> Bool {
        guard state.isFocused else {
            return false
        }
        guard state != appliedState || windowNeedsChrome(state, window: window) else {
            return false
        }
        let titleChanged = applyFocusedChrome(state, to: window)
        if titleChanged {
            NotificationCenter.default.post(name: .termyNativeTabsChanged, object: nil)
        }
        appliedState = state
        return true
    }

    @discardableResult
    static func applyFocusedChrome(_ state: TerminalWindowChromeState, to window: NSWindow) -> Bool {
        applyFocusedChrome(state, to: window, updatesTitle: true)
    }

    @discardableResult
    static func applyFocusedChrome(
        _ state: TerminalWindowChromeState,
        to window: NSWindow,
        updatesTitle: Bool
    ) -> Bool {
        var titleChanged = false
        if updatesTitle, window.title != state.title {
            window.title = state.title
            titleChanged = true
        }
        window.titlebarAppearsTransparent = true
        window.backgroundColor = state.chromeBackgroundColor
        TerminalWindowBlur.sync(state, in: window)
        let isOpaque = !state.requiresTransparentWindow
        if window.isOpaque != isOpaque {
            window.isOpaque = isOpaque
            // Toggling opacity changes the window's compositing and shadow;
            // refresh it so a stale outline doesn't linger when blur flips.
            window.invalidateShadow()
        }
        if let titlebarTabsWindow = window as? TitlebarTabsWindow {
            titlebarTabsWindow.refreshTitlebarTabsLayout()
        }
        return titleChanged
    }

    static func windowNeedsChrome(
        _ state: TerminalWindowChromeState,
        window: NSWindow,
        checksTitle: Bool = true
    ) -> Bool {
        if checksTitle && window.title != state.title {
            return true
        }

        if !window.titlebarAppearsTransparent
            || window.isOpaque == state.requiresTransparentWindow
        {
            return true
        }

        if TerminalWindowBlur.shouldInstall(for: state) != TerminalWindowBlur.contains(in: window) {
            return true
        }

        let expectedColor = state.chromeBackgroundColor.usingColorSpace(.sRGB)
        let actualColor = window.backgroundColor.usingColorSpace(.sRGB)
        guard let expectedColor, let actualColor else {
            return true
        }
        return abs(actualColor.redComponent - expectedColor.redComponent) > 0.001
            || abs(actualColor.greenComponent - expectedColor.greenComponent) > 0.001
            || abs(actualColor.blueComponent - expectedColor.blueComponent) > 0.001
            || abs(actualColor.alphaComponent - expectedColor.alphaComponent) > 0.001
    }
}

/// Background blur via the window server (the same private CGS call Ghostty
/// and Terminal.app-style transparency use). Unlike an `NSVisualEffectView`,
/// it blurs everything behind the window's transparent pixels — titlebar
/// included — with no view-hierarchy or frame-tracking games, and shows the
/// actual desktop instead of a translucent material wash.
@MainActor
enum TerminalWindowBlur {
    private static let blurRadius = 20

    /// Blur state we asked the window server for, per window. Weak keys so
    /// closed windows drop out; also lets tests observe intent for windows
    /// that were never ordered on screen (no valid window number).
    private static let appliedBlur = NSMapTable<NSWindow, NSNumber>.weakToStrongObjects()

    static func shouldInstall(for state: TerminalWindowChromeState) -> Bool {
        state.backgroundBlur && state.effectiveBackgroundAlpha < 0.999
    }

    static func contains(in window: NSWindow) -> Bool {
        appliedBlur.object(forKey: window)?.boolValue ?? false
    }

    static func sync(_ state: TerminalWindowChromeState, in window: NSWindow) {
        let enabled = shouldInstall(for: state)
        // Re-issue the CGS call even when intent is unchanged: the window may
        // not have had a window-server number when we first recorded it.
        if window.windowNumber > 0 {
            _ = CGSSetWindowBackgroundBlurRadius(
                CGSMainConnectionID(),
                window.windowNumber,
                enabled ? blurRadius : 0
            )
        }
        appliedBlur.setObject(NSNumber(value: enabled), forKey: window)
    }
}

@_silgen_name("CGSMainConnectionID")
private func CGSMainConnectionID() -> Int32

@_silgen_name("CGSSetWindowBackgroundBlurRadius")
private func CGSSetWindowBackgroundBlurRadius(_ cid: Int32, _ wid: Int, _ radius: Int) -> Int32

private final class TerminalWindowChromeSyncNSView: NSView {
    private var pendingState: TerminalWindowChromeState?
    private var appliedState: TerminalWindowChromeState?

    func syncChrome(_ state: TerminalWindowChromeState) {
        pendingState = state
        applyPendingChromeIfPossible()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        applyPendingChromeIfPossible()
    }

    private func applyPendingChromeIfPossible() {
        guard let state = pendingState, state.isFocused, let window else {
            return
        }
        if NativeTabWindowManager.shared.applyTerminalChrome(state, for: window, requireVisible: true) {
            appliedState = state
            return
        }
        TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState)
    }
}

private struct TerminalScrollBar: View {
    let frameMetadata: TerminalFrameMetadata
    let renderConfig: TerminalRenderConfig
    let searchMatches: [TerminalSearchMatch]
    let commandMarks: [Int]
    let onInteraction: () -> Void
    let onScrollToOffset: (Int) -> Void

    var body: some View {
        GeometryReader { proxy in
            if metrics(for: proxy.size).isVisible {
                ZStack(alignment: .top) {
                    Capsule()
                        .fill(trackColor)
                        .frame(width: 7)

                    let prompts = markerYs(rows: commandMarks, for: proxy.size)
                    ForEach(prompts.indices, id: \.self) { index in
                        Capsule()
                            .fill(renderConfig.foreground.swiftUIColor.opacity(0.4))
                            .frame(width: 7, height: 1)
                            .offset(y: prompts[index])
                    }

                    let markers = markerYs(rows: searchMatches.map(\.row), for: proxy.size)
                    ForEach(markers.indices, id: \.self) { index in
                        Capsule()
                            .fill(Color.accentColor.opacity(0.85))
                            .frame(width: 7, height: 2)
                            .offset(y: markers[index])
                    }

                    Capsule()
                        .fill(thumbColor)
                        .frame(width: 7, height: metrics(for: proxy.size).thumbHeight)
                        .offset(y: metrics(for: proxy.size).thumbY)
                        .gesture(
                            DragGesture(minimumDistance: 0)
                                .onChanged { value in
                                    onInteraction()
                                    let metrics = metrics(for: proxy.size)
                                    guard metrics.travel > 0 else {
                                        return
                                    }
                                    let clampedY = max(0, min(value.location.y, metrics.travel))
                                    let progressFromTop = clampedY / metrics.travel
                                    let target = Int(round(CGFloat(frameMetadata.historySize) * (1 - progressFromTop)))
                                    onScrollToOffset(target)
                                }
                        )
                }
                .padding(.vertical, framePaddingY)
                .contentShape(Rectangle())
            }
        }
        .allowsHitTesting(frameMetadata.historySize > 0)
    }

    private var framePaddingY: CGFloat {
        8
    }

    /// Y offsets (within the padded track) for the given absolute scrollback
    /// rows, so marks/hits show as buckets along the scrollbar.
    private func markerYs(rows: [Int], for size: CGSize) -> [CGFloat] {
        let totalRows = CGFloat(frameMetadata.historySize + frameMetadata.rows)
        guard totalRows > 0, size.height > framePaddingY * 2, !rows.isEmpty else {
            return []
        }
        let trackHeight = size.height - (framePaddingY * 2)
        let unique = Set(rows).filter { $0 >= 0 }
        return unique.map { trackHeight * min(1, CGFloat($0) / totalRows) }.sorted()
    }

    private var trackColor: Color {
        switch renderConfig.scrollbarStyle {
        case .neutral:
            return Color.primary.opacity(0.10)
        case .mutedTheme:
            return renderConfig.foreground.swiftUIColor.opacity(0.08)
        case .theme:
            return renderConfig.cursor.swiftUIColor.opacity(0.16)
        }
    }

    private var thumbColor: Color {
        let opacity = frameMetadata.displayOffset == 0 ? 0.34 : 0.58
        switch renderConfig.scrollbarStyle {
        case .neutral:
            return Color.primary.opacity(opacity)
        case .mutedTheme:
            return renderConfig.foreground.swiftUIColor.opacity(opacity * 0.75)
        case .theme:
            return renderConfig.cursor.swiftUIColor.opacity(max(0.45, opacity))
        }
    }

    private func metrics(for size: CGSize) -> ScrollBarMetrics {
        guard frameMetadata.historySize > 0, frameMetadata.rows > 0, size.height > framePaddingY * 2 else {
            return ScrollBarMetrics(isVisible: false, thumbHeight: 0, thumbY: 0, travel: 0)
        }

        let trackHeight = size.height - (framePaddingY * 2)
        let totalRows = max(1, frameMetadata.historySize + frameMetadata.rows)
        let visibleFraction = CGFloat(frameMetadata.rows) / CGFloat(totalRows)
        let thumbHeight = max(28, min(trackHeight, trackHeight * visibleFraction))
        let travel = max(0, trackHeight - thumbHeight)
        let maxOffset = max(1, frameMetadata.historySize)
        let normalizedOffset = CGFloat(max(0, min(frameMetadata.displayOffset, frameMetadata.historySize)))
            / CGFloat(maxOffset)
        let thumbY = travel * (1 - normalizedOffset)

        return ScrollBarMetrics(
            isVisible: true,
            thumbHeight: thumbHeight,
            thumbY: thumbY,
            travel: travel
        )
    }
}

private struct ScrollBarMetrics {
    var isVisible: Bool
    var thumbHeight: CGFloat
    var thumbY: CGFloat
    var travel: CGFloat
}
