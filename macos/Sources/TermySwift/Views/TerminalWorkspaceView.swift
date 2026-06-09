import AppKit
import SwiftUI

struct TerminalWorkspaceView: View {
    @StateObject private var store: TerminalWorkspaceStore
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared
    @State private var appConfigurationError = TermyAppConfiguration.loadErrorMessage
    @State private var workspacePersistenceError: String?
    @State private var didRestoreWorkspace = false
    @State private var persistenceSaveTask: Task<Void, Never>?
    @State private var currentWindow: NSWindow?
    private let workspacePersistence = TerminalWorkspacePersistence()
    private let shouldRestorePersistedWorkspace: Bool

    init(initialTask: TermyTaskConfiguration? = nil) {
        _store = StateObject(wrappedValue: TerminalWorkspaceStore(initialTask: initialTask))
        shouldRestorePersistedWorkspace = initialTask == nil
    }

    var body: some View {
        tabbedWorkspaceContent
            .background(TerminalWorkspaceRoutingView(
                store: store,
                onWindowChanged: { window in
                    currentWindow = window
                }
            ))
            .focusedValue(\.terminalCommands, commandSet)
            .onAppear {
                TerminalCommandRouter.shared.activate(store)
                restoreWorkspaceIfNeeded()
            }
            .onDisappear {
                persistWorkspace()
            }
            .onReceive(store.objectWillChange) { _ in
                scheduleWorkspacePersistence()
            }
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.willTerminateNotification)) { _ in
                persistWorkspace()
            }
            .onReceive(configurationStore.$loadErrorMessage) { message in
                appConfigurationError = message
            }
    }

    @ViewBuilder
    private var tabbedWorkspaceContent: some View {
        let configuration = configurationStore.configuration
        switch configuration.native.tabBarPosition {
        case .top:
            VStack(spacing: 0) {
                NativeTabChromeView(window: currentWindow, configuration: configuration)
                workspaceContent
            }
        case .right:
            HStack(spacing: 0) {
                workspaceContent
                NativeTabChromeView(window: currentWindow, configuration: configuration)
            }
        }
    }

    private var workspaceContent: some View {
        ZStack {
            if let zoomedPane = store.zoomedPane {
                TerminalPaneLeafView(pane: zoomedPane, store: store)
            } else {
                TerminalPaneNodeView(node: store.root, store: store)
            }

            if let appConfigurationError {
                dismissibleBanner(appConfigurationError, color: .red) {
                    self.appConfigurationError = nil
                }
            }

            if let workspacePersistenceError {
                dismissibleBanner(workspacePersistenceError, color: .orange) {
                    self.workspacePersistenceError = nil
                }
            }

            if store.isSearchVisible, store.isSearchInputFocused {
                Color.clear
                    .contentShape(Rectangle())
                    .onTapGesture {
                        store.setSearchInputFocused(false)
                    }
                    .zIndex(9)
            }

            if store.isSearchVisible, let terminal = store.focusedTerminal {
                TerminalSearchPanel(
                    terminal: terminal,
                    options: $store.searchOptions,
                    focusRequest: store.searchFocusRequest,
                    onFocusChanged: store.setSearchInputFocused,
                    onClose: store.hideSearch
                )
                .padding(10)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .zIndex(10)
            }

            if store.isCommandPaletteVisible {
                commandPaletteOverlay
                    .zIndex(12)
            }

            TermyToastOverlay()
                .zIndex(20)
        }
    }

    /// A top-leading error banner with a dismiss button, overlaid on the workspace.
    private func dismissibleBanner(
        _ message: String,
        color: Color,
        onDismiss: @escaping () -> Void
    ) -> some View {
        HStack(spacing: 8) {
            Text(message)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(color)
            Button(action: onDismiss) {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
        }
        .padding(8)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .zIndex(11)
    }

    private var commandPaletteOverlay: some View {
        ZStack {
            Color.black.opacity(0.12)
                .ignoresSafeArea()
                .contentShape(Rectangle())
                .onTapGesture {
                    store.hideCommandPalette()
                }

            VStack(spacing: 0) {
                TerminalCommandPalette(
                    commandSet: commandSet,
                    configuration: configurationStore.configuration,
                    onClose: store.hideCommandPalette
                )
                Spacer(minLength: 0)
            }
            .padding(.top, 60)
        }
    }

    private var commandSet: TerminalCommandSet {
        TerminalCommandSet(
            newTab: {
                NativeTabWindowManager.shared.openNativeTab()
            },
            closePaneOrTab: {
                if !store.closeFocusedPaneIfSplit() {
                    NSApp.keyWindow?.performClose(nil)
                }
                scheduleWorkspacePersistence()
            },
            splitRight: {
                store.splitFocused(.horizontal)
                scheduleWorkspacePersistence()
            },
            splitDown: {
                store.splitFocused(.vertical)
                scheduleWorkspacePersistence()
            },
            closePane: {
                store.closeFocusedPane()
                scheduleWorkspacePersistence()
            },
            focusPane: { direction in
                _ = store.focusPane(in: direction)
            },
            focusNextPane: store.focusNextPane,
            focusPreviousPane: store.focusPreviousPane,
            resizePane: { direction in
                if store.resizeFocusedPane(in: direction) {
                    scheduleWorkspacePersistence()
                }
            },
            togglePaneZoom: store.toggleFocusedPaneZoom,
            increaseFontSize: {
                store.focusedTerminal?.increaseFontSize()
            },
            decreaseFontSize: {
                store.focusedTerminal?.decreaseFontSize()
            },
            resetFontSize: {
                store.focusedTerminal?.resetFontSize()
            },
            copy: {
                store.focusedTerminal?.copySelection() ?? false
            },
            paste: {
                guard let text = NSPasteboard.general.string(forType: .string) else {
                    return
                }
                store.focusedTerminal?.send(bytes: Array(text.utf8))
            },
            clearScrollback: {
                store.focusedTerminal?.clearScrollback()
            },
            showSearch: store.showSearch,
            hideSearch: store.hideSearch,
            searchNext: {
                store.focusedTerminal?.selectNextSearchMatch()
            },
            searchPrevious: {
                store.focusedTerminal?.selectPreviousSearchMatch()
            },
            toggleSearchCaseSensitive: {
                store.toggleSearchCaseSensitive()
            },
            toggleSearchRegex: {
                store.toggleSearchRegex()
            },
            sendInterrupt: { store.focusedTerminal?.sendControlC() },
            toggleCommandPalette: store.toggleCommandPalette
        )
    }

    private func restoreWorkspaceIfNeeded() {
        guard !didRestoreWorkspace else {
            return
        }
        didRestoreWorkspace = true
        let native = configurationStore.configuration.native
        guard native.nativeTabPersistence || native.nativeLayoutAutosave else {
            workspacePersistenceError = nil
            return
        }
        guard shouldRestorePersistedWorkspace else {
            workspacePersistenceError = nil
            return
        }

        do {
            let snapshot = try native.nativeTabPersistence
                ? workspacePersistence.loadLastSession()
                : workspacePersistence.loadAutosavedLayout()
            if store.restore(from: snapshot) {
                workspacePersistenceError = nil
            }
        } catch TerminalWorkspacePersistenceError.missingLastSession {
            workspacePersistenceError = nil
        } catch {
            workspacePersistenceError = "Could not restore workspace: \(error)"
        }
    }

    private func scheduleWorkspacePersistence() {
        guard didRestoreWorkspace,
              shouldPersistWorkspace
        else {
            return
        }
        persistenceSaveTask?.cancel()
        persistenceSaveTask = Task {
            try? await Task.sleep(nanoseconds: 250_000_000)
            guard !Task.isCancelled else {
                return
            }
            await MainActor.run {
                persistWorkspace()
            }
        }
    }

    private func persistWorkspace() {
        guard didRestoreWorkspace,
              shouldPersistWorkspace
        else {
            return
        }
        do {
            let native = configurationStore.configuration.native
            let snapshot = store.snapshot(includeBuffers: native.nativeBufferPersistence)
            if native.nativeTabPersistence {
                try workspacePersistence.saveLastSession(snapshot)
            }
            if native.nativeLayoutAutosave {
                try workspacePersistence.saveAutosavedLayout(snapshot)
            }
            workspacePersistenceError = nil
        } catch {
            workspacePersistenceError = "Could not save workspace: \(error)"
        }
    }

    private var shouldPersistWorkspace: Bool {
        let native = configurationStore.configuration.native
        return native.nativeTabPersistence || native.nativeLayoutAutosave
    }

}

private struct TerminalCommandPalette: View {
    let commandSet: TerminalCommandSet
    let configuration: TermyAppConfiguration
    let onClose: () -> Void

    @State private var query = ""
    @State private var selectedIndex = 0
    @FocusState private var isSearchFocused: Bool

    /// Commands matching the query, ranked by fuzzy score (ties keep the
    /// catalog order).
    private var filteredCommands: [(command: PaletteCommand, match: CommandPaletteMatch)] {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let matches = paletteCommands.compactMap { command in
            CommandPaletteFilter.match(query: needle, title: command.title, action: command.action.identifier)
                .map { (command: command, match: $0) }
        }
        guard !needle.isEmpty else {
            return matches
        }
        return matches.enumerated()
            .sorted { lhs, rhs in
                if lhs.element.match.score != rhs.element.match.score {
                    return lhs.element.match.score > rhs.element.match.score
                }
                return lhs.offset < rhs.offset
            }
            .map(\.element)
    }

    var body: some View {
        let filtered = filteredCommands
        let clampedSelection = min(selectedIndex, max(0, filtered.count - 1))

        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "command")
                    .foregroundStyle(.secondary)
                TextField("Type a command…", text: $query)
                    .textFieldStyle(.plain)
                    .focused($isSearchFocused)
                    .onSubmit {
                        execute(filtered[safe: clampedSelection]?.command)
                    }
                    .onExitCommand {
                        onClose()
                    }
                    .onKeyPress(.downArrow) {
                        selectedIndex = min(clampedSelection + 1, max(0, filtered.count - 1))
                        return .handled
                    }
                    .onKeyPress(.upArrow) {
                        selectedIndex = max(clampedSelection - 1, 0)
                        return .handled
                    }
                Button {
                    onClose()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.borderless)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)

            Divider()

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 1) {
                        ForEach(Array(filtered.enumerated()), id: \.element.command.id) { index, entry in
                            paletteRow(
                                entry.command,
                                match: entry.match,
                                isSelected: index == clampedSelection
                            )
                            .id(entry.command.id)
                            .onHover { hovering in
                                if hovering {
                                    selectedIndex = index
                                }
                            }
                        }

                        if filtered.isEmpty {
                            Text("No matching commands")
                                .foregroundStyle(.secondary)
                                .padding(.vertical, 14)
                        }
                    }
                    .padding(6)
                }
                .frame(maxHeight: 320)
                .onChange(of: clampedSelection) { _, index in
                    guard let id = filtered[safe: index]?.command.id else {
                        return
                    }
                    proxy.scrollTo(id)
                }
            }
        }
        .frame(width: 430)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        .overlay {
            RoundedRectangle(cornerRadius: 10)
                .stroke(.separator.opacity(0.8), lineWidth: 1)
        }
        .shadow(radius: 18)
        .onChange(of: query) { _, _ in
            selectedIndex = 0
        }
        .onAppear {
            isSearchFocused = true
        }
        .task {
            // The terminal's keyboard view may still hold first responder
            // when the palette mounts; re-assert once the window settles
            // (same pattern as the tab rename sheet).
            try? await Task.sleep(nanoseconds: 10_000_000)
            isSearchFocused = true
        }
    }

    private func paletteRow(
        _ command: PaletteCommand,
        match: CommandPaletteMatch,
        isSelected: Bool
    ) -> some View {
        Button {
            execute(command)
        } label: {
            HStack(spacing: 10) {
                Image(systemName: command.systemImage)
                    .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                    .frame(width: 18)

                Text(highlightedTitle(command.title, matchedIndices: match.matchedTitleIndices))
                    .lineLimit(1)

                Spacer()

                if configuration.native.commandPaletteShowKeybinds,
                   let shortcut = shortcutLabel(for: command.action) {
                    Text(shortcut)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .background(
            isSelected ? Color.accentColor.opacity(0.16) : Color.clear,
            in: RoundedRectangle(cornerRadius: 6)
        )
    }

    private func highlightedTitle(_ title: String, matchedIndices: [Int]) -> AttributedString {
        var attributed = AttributedString(title)
        for offset in matchedIndices {
            guard offset < title.count else {
                continue
            }
            let start = attributed.index(attributed.startIndex, offsetByCharacters: offset)
            let end = attributed.index(start, offsetByCharacters: 1)
            attributed[start..<end].inlinePresentationIntent = .stronglyEmphasized
            attributed[start..<end].foregroundColor = .accentColor
        }
        return attributed
    }

    private func execute(_ command: PaletteCommand?) {
        guard let command else {
            return
        }
        onClose()
        command.execute(commandSet)
    }

    private func shortcutLabel(for action: TerminalKeybindAction) -> String? {
        guard let keybind = configuration.keybinds.first(where: { $0.keybindAction == action }) else {
            return nil
        }
        return keybind.trigger
            .replacingOccurrences(of: "secondary", with: "cmd")
            .replacingOccurrences(of: "cmd", with: "⌘")
            .replacingOccurrences(of: "ctrl", with: "⌃")
            .replacingOccurrences(of: "alt", with: "⌥")
            .replacingOccurrences(of: "shift", with: "⇧")
            .replacingOccurrences(of: "-", with: " ")
    }

    private var paletteCommands: [PaletteCommand] {
        [
            PaletteCommand(title: "New Tab", action: .newTab, systemImage: "plus") { $0.execute(.newTab) },
            PaletteCommand(title: "Switch Tab Left", action: .switchTabLeft, systemImage: "chevron.left") { _ in
                NativeTabWindowManager.shared.selectRelativeNativeTab(offset: -1)
            },
            PaletteCommand(title: "Switch Tab Right", action: .switchTabRight, systemImage: "chevron.right") { _ in
                NativeTabWindowManager.shared.selectRelativeNativeTab(offset: 1)
            },
            PaletteCommand(title: "Move Tab Left", action: .moveTabLeft, systemImage: "arrow.left.to.line") { _ in
                NativeTabWindowManager.shared.moveSelectedNativeTab(offset: -1)
            },
            PaletteCommand(title: "Move Tab Right", action: .moveTabRight, systemImage: "arrow.right.to.line") { _ in
                NativeTabWindowManager.shared.moveSelectedNativeTab(offset: 1)
            },
            PaletteCommand(title: "Split Right", action: .splitPaneVertical, systemImage: "rectangle.split.2x1") { $0.execute(.splitPaneVertical) },
            PaletteCommand(title: "Split Down", action: .splitPaneHorizontal, systemImage: "rectangle.split.1x2") { $0.execute(.splitPaneHorizontal) },
            PaletteCommand(title: "Close Pane or Tab", action: .closePaneOrTab, systemImage: "xmark") { $0.execute(.closePaneOrTab) },
            PaletteCommand(title: "Close Pane", action: .closePane, systemImage: "rectangle.badge.xmark") { $0.execute(.closePane) },
            PaletteCommand(title: "Next Pane", action: .focusPaneNext, systemImage: "arrow.right") { $0.execute(.focusPaneNext) },
            PaletteCommand(title: "Previous Pane", action: .focusPanePrevious, systemImage: "arrow.left") { $0.execute(.focusPanePrevious) },
            PaletteCommand(title: "Toggle Pane Zoom", action: .togglePaneZoom, systemImage: "arrow.up.left.and.arrow.down.right") { $0.execute(.togglePaneZoom) },
            PaletteCommand(title: "Increase Font Size", action: .increaseFontSize, systemImage: "textformat.size.larger") { $0.execute(.increaseFontSize) },
            PaletteCommand(title: "Decrease Font Size", action: .decreaseFontSize, systemImage: "textformat.size.smaller") { $0.execute(.decreaseFontSize) },
            PaletteCommand(title: "Reset Font Size", action: .resetFontSize, systemImage: "textformat") { $0.execute(.resetFontSize) },
            PaletteCommand(title: "Find", action: .openSearch, systemImage: "magnifyingglass") { $0.execute(.openSearch) },
            PaletteCommand(title: "Find Next", action: .searchNext, systemImage: "chevron.down") { $0.execute(.searchNext) },
            PaletteCommand(title: "Find Previous", action: .searchPrevious, systemImage: "chevron.up") { $0.execute(.searchPrevious) },
            PaletteCommand(title: "Toggle Case Sensitive Search", action: .toggleSearchCaseSensitive, systemImage: "textformat") { $0.execute(.toggleSearchCaseSensitive) },
            PaletteCommand(title: "Toggle Regex Search", action: .toggleSearchRegex, systemImage: "asterisk") { $0.execute(.toggleSearchRegex) },
            PaletteCommand(title: "Copy", action: .copy, systemImage: "doc.on.doc") { $0.execute(.copy) },
            PaletteCommand(title: "Paste", action: .paste, systemImage: "doc.on.clipboard") { $0.execute(.paste) },
            PaletteCommand(title: "Clear Scrollback", action: .clearScrollback, systemImage: "trash") { $0.execute(.clearScrollback) },
            PaletteCommand(title: "Send Interrupt", action: .sendInterrupt, systemImage: "exclamationmark.octagon") { $0.execute(.sendInterrupt) },
            PaletteCommand(title: "Open Config", action: .openConfig, systemImage: "doc.text") { _ in
                _ = TermyNativeAppActions.openConfigFileInEditor()
            },
            PaletteCommand(title: "Prettify Config", action: .prettifyConfig, systemImage: "wand.and.stars") { _ in
                _ = TermyNativeAppActions.prettifyConfig()
            },
            PaletteCommand(title: "Toggle Native Tab Bar", action: .toggleTabBarVisibility, systemImage: "sidebar.left") { _ in
                _ = TermyNativeAppActions.toggleNativeTabBarVisibility(for: NSApp.keyWindow)
            },
            PaletteCommand(title: "App Info", action: .appInfo, systemImage: "info.circle") { _ in
                TermyNativeAppActions.showAppInfo()
            },
            PaletteCommand(title: "Restart App", action: .restartApp, systemImage: "arrow.clockwise") { _ in
                TermyNativeAppActions.restartApp()
            },
        ] + configuration.tasks.map { task in
            PaletteCommand(title: "Run \(task.name)", action: .runTask, systemImage: "play") { _ in
                NativeTabWindowManager.shared.openNativeTab(startupTask: task)
            }
        }
    }
}

private struct PaletteCommand: Identifiable {
    let title: String
    let action: TerminalKeybindAction
    let systemImage: String
    let execute: (TerminalCommandSet) -> Void

    /// Stable across renders (the command list is recomputed per body
    /// evaluation) so ForEach identity, selection, and scroll targets hold.
    /// Task commands share the "run_task" action, hence the title suffix.
    var id: String {
        "\(action.identifier):\(title)"
    }
}

private struct TerminalWorkspaceRoutingView: NSViewRepresentable {
    @ObservedObject var store: TerminalWorkspaceStore
    let onWindowChanged: (NSWindow?) -> Void

    func makeNSView(context: Context) -> RoutingRegistrationView {
        RoutingRegistrationView(store: store, onWindowChanged: onWindowChanged)
    }

    func updateNSView(_ view: RoutingRegistrationView, context: Context) {
        view.store = store
        view.onWindowChanged = onWindowChanged
        view.registerCurrentWindow()
    }
}

private final class RoutingRegistrationView: NSView {
    weak var store: TerminalWorkspaceStore?
    private weak var registeredWindow: NSWindow?
    var onWindowChanged: (NSWindow?) -> Void

    init(store: TerminalWorkspaceStore, onWindowChanged: @escaping (NSWindow?) -> Void) {
        self.store = store
        self.onWindowChanged = onWindowChanged
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        nil
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        onWindowChanged(window)
        registerCurrentWindow()
    }

    func registerCurrentWindow() {
        if let registeredWindow, registeredWindow !== window {
            TerminalCommandRouter.shared.unregister(window: registeredWindow)
            self.registeredWindow = nil
        }

        guard let window, let store else {
            onWindowChanged(window)
            return
        }
        registeredWindow = window
        TerminalCommandRouter.shared.register(store, for: window)
        onWindowChanged(window)
    }
}

private struct TerminalPaneNodeView: View {
    @ObservedObject var node: TerminalPaneNode
    @ObservedObject var store: TerminalWorkspaceStore

    var body: some View {
        switch node.kind {
        case .leaf(let pane):
            TerminalPaneLeafView(pane: pane, store: store)
        case .split(let axis, let first, let second):
            StableSplitView(
                axis: axis,
                ratio: node.splitRatio
            ) {
                TerminalPaneNodeView(node: first, store: store)
            } second: {
                TerminalPaneNodeView(node: second, store: store)
            }
        }
    }
}

private struct StableSplitView<First: View, Second: View>: NSViewControllerRepresentable {
    let axis: TerminalSplitAxis
    let ratio: Double
    let first: First
    let second: Second

    init(
        axis: TerminalSplitAxis,
        ratio: Double,
        @ViewBuilder first: () -> First,
        @ViewBuilder second: () -> Second
    ) {
        self.axis = axis
        self.ratio = ratio
        self.first = first()
        self.second = second()
    }

    func makeNSViewController(context: Context) -> StableSplitViewController<First, Second> {
        StableSplitViewController(
            axis: axis,
            ratio: ratio,
            first: first,
            second: second
        )
    }

    func updateNSViewController(
        _ splitViewController: StableSplitViewController<First, Second>,
        context: Context
    ) {
        splitViewController.update(axis: axis, ratio: ratio, first: first, second: second)
    }
}

private final class StableSplitViewController<First: View, Second: View>: NSSplitViewController {
    private let firstHostingController: NSHostingController<First>
    private let secondHostingController: NSHostingController<Second>
    private var didApplyInitialDividerPosition = false
    private var axis: TerminalSplitAxis
    private var ratio: Double

    init(
        axis: TerminalSplitAxis,
        ratio: Double,
        first: First,
        second: Second
    ) {
        self.axis = axis
        self.ratio = ratio
        firstHostingController = NSHostingController(rootView: first)
        secondHostingController = NSHostingController(rootView: second)
        super.init(nibName: nil, bundle: nil)

        splitView = StableDividerSplitView()
        splitView.dividerStyle = .thin
        splitView.isVertical = axis == .horizontal

        addSplitViewItem(item(for: firstHostingController))
        addSplitViewItem(item(for: secondHostingController))
        updateMinimumThickness()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func update(axis: TerminalSplitAxis, ratio: Double, first: First, second: Second) {
        firstHostingController.rootView = first
        secondHostingController.rootView = second

        if self.axis != axis {
            self.axis = axis
            splitView.isVertical = axis == .horizontal
            splitView.window?.invalidateCursorRects(for: splitView)
            didApplyInitialDividerPosition = false
            updateMinimumThickness()
        }

        if abs(self.ratio - ratio) > 0.0001 {
            self.ratio = ratio
            didApplyInitialDividerPosition = false
        }
    }

    override func viewDidLayout() {
        super.viewDidLayout()
        applyInitialDividerPositionIfNeeded()
    }

    private func item<Content: View>(for hostingController: NSHostingController<Content>) -> NSSplitViewItem {
        let item = NSSplitViewItem(viewController: hostingController)
        item.canCollapse = false
        item.holdingPriority = .defaultLow
        return item
    }

    private func updateMinimumThickness() {
        let minimum: CGFloat = axis == .horizontal ? 220 : 120
        splitViewItems.forEach { item in
            item.minimumThickness = minimum
        }
    }

    private func applyInitialDividerPositionIfNeeded() {
        guard !didApplyInitialDividerPosition else {
            return
        }

        let length = splitView.isVertical ? splitView.bounds.width : splitView.bounds.height
        guard length > 0 else {
            return
        }

        splitView.setPosition(length * ratio, ofDividerAt: 0)
        didApplyInitialDividerPosition = true
    }

    // Panes are intentionally NOT suspended while the divider drags or the
    // window resizes: every resize step re-grids and re-presents the affected
    // terminals live, so content tracks the divider/window edge instead of
    // freezing on the stale grid and snapping after the drag ends. Per-step
    // cost is bounded — `TerminalViewModel.resize` only refreshes when the
    // col/row count actually changes, and the refresh itself is throttled.
    // Occluded background tabs are still suspended via the window occlusion
    // path in TermySwiftApp.
    override func splitViewDidResizeSubviews(_ notification: Notification) {
        splitView.window?.invalidateCursorRects(for: splitView)
    }

    override func splitView(
        _ splitView: NSSplitView,
        effectiveRect proposedEffectiveRect: NSRect,
        forDrawnRect drawnRect: NSRect,
        ofDividerAt dividerIndex: Int
    ) -> NSRect {
        guard let splitView = splitView as? StableDividerSplitView else {
            return proposedEffectiveRect
        }
        return splitView.expandedDividerRect(forDrawnRect: drawnRect)
    }
}

private final class StableDividerSplitView: NSSplitView {
    private static let dividerHitThickness: CGFloat = 12

    override func resetCursorRects() {
        super.resetCursorRects()

        for dividerIndex in 0..<max(0, arrangedSubviews.count - 1) {
            let rect = expandedDividerRect(ofDividerAt: dividerIndex)
            addCursorRect(rect, cursor: resizeCursor)
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        for dividerIndex in 0..<max(0, arrangedSubviews.count - 1) {
            if expandedDividerRect(ofDividerAt: dividerIndex).contains(point) {
                return self
            }
        }
        return super.hitTest(point)
    }

    override func mouseMoved(with event: NSEvent) {
        if isEventInsideExpandedDivider(event) {
            resizeCursor.set()
            return
        }
        super.mouseMoved(with: event)
    }

    override func mouseDown(with event: NSEvent) {
        if isEventInsideExpandedDivider(event) {
            resizeCursor.set()
        }
        super.mouseDown(with: event)
    }

    override var isVertical: Bool {
        didSet {
            window?.invalidateCursorRects(for: self)
        }
    }

    private var resizeCursor: NSCursor {
        isVertical ? .resizeLeftRight : .resizeUpDown
    }

    private func isEventInsideExpandedDivider(_ event: NSEvent) -> Bool {
        let point = convert(event.locationInWindow, from: nil)
        for dividerIndex in 0..<max(0, arrangedSubviews.count - 1) {
            if expandedDividerRect(ofDividerAt: dividerIndex).contains(point) {
                return true
            }
        }
        return false
    }

    func expandedDividerRect(forDrawnRect drawnRect: NSRect) -> NSRect {
        expandedDividerRect(expanding: drawnRect)
    }

    private func expandedDividerRect(ofDividerAt dividerIndex: Int) -> NSRect {
        guard dividerIndex >= 0, dividerIndex + 1 < arrangedSubviews.count else {
            return .zero
        }

        let leadingFrame = arrangedSubviews[dividerIndex].frame
        let trailingFrame = arrangedSubviews[dividerIndex + 1].frame
        let targetThickness = Self.dividerHitThickness
        var rect: NSRect

        if isVertical {
            let centerX = (leadingFrame.maxX + trailingFrame.minX) / 2
            rect = NSRect(
                x: centerX - (targetThickness / 2),
                y: bounds.minY,
                width: targetThickness,
                height: bounds.height
            )
        } else {
            let centerY = (leadingFrame.maxY + trailingFrame.minY) / 2
            rect = NSRect(
                x: bounds.minX,
                y: centerY - (targetThickness / 2),
                width: bounds.width,
                height: targetThickness
            )
        }

        return expandedDividerRect(expanding: rect)
    }

    private func expandedDividerRect(expanding rect: NSRect) -> NSRect {
        var rect = rect
        let targetThickness = Self.dividerHitThickness

        if isVertical {
            let delta = max(0, targetThickness - rect.width)
            rect.origin.x -= delta / 2
            rect.size.width += delta
        } else {
            let delta = max(0, targetThickness - rect.height)
            rect.origin.y -= delta / 2
            rect.size.height += delta
        }

        return rect.intersection(bounds)
    }
}

private struct TerminalPaneLeafView: View {
    @ObservedObject var pane: TerminalPane
    @ObservedObject var store: TerminalWorkspaceStore
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared

    var body: some View {
        ZStack(alignment: .topTrailing) {
            TerminalSurfaceView(
                terminal: pane.terminal,
                isFocused: store.focusedPaneID == pane.id,
                showsFocusBorder: store.paneCount > 1,
                // While an overlay owns the keyboard, the terminal must not
                // re-steal first responder (updateNSView refocuses on every
                // frame tick, which makes overlay text fields untypable).
                isInputEnabled: !store.isSearchInputFocused && !store.isCommandPaletteVisible,
                isSearchVisible: store.isSearchVisible,
                windowTitle: store.tabDisplayTitle,
                onFocus: {
                    store.focus(pane)
                },
                onSplitRight: {
                    store.focus(pane)
                    store.splitFocused(.horizontal)
                },
                onSplitDown: {
                    store.focus(pane)
                    store.splitFocused(.vertical)
                },
                onClosePane: {
                    store.focus(pane)
                    store.closeFocusedPane()
                },
                onClosePaneIfSplit: {
                    store.focus(pane)
                    return store.closeFocusedPaneIfSplit()
                },
                onFocusNextPane: store.focusNextPane,
                onShowSearch: store.showSearch,
                onDismissSearch: {
                    store.setSearchInputFocused(false)
                }
            )

            if configurationStore.configuration.native.showDebugOverlay {
                TerminalDebugOverlay(metrics: pane.terminal.debugMetrics)
                    .padding(8)
                    .allowsHitTesting(false)
            }
        }
        .id(pane.id)
        .frame(minWidth: 240, minHeight: 120)
    }
}

private struct TerminalDebugOverlay: View {
    let metrics: TerminalDebugMetrics

    var body: some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text("\(metrics.framesPerSecond, specifier: "%.0f") FPS")
            Text("\(metrics.cpuPercent, specifier: "%.0f")% CPU")
            Text("\(metrics.memoryMegabytes, specifier: "%.0f") MB")
            Text("\(metrics.skippedPresentsPerSecond, specifier: "%.0f") skip")
            Text("\(metrics.fullRebuildsPerSecond, specifier: "%.0f")/\(metrics.partialRebuildsPerSecond, specifier: "%.0f") full/part")
        }
        .font(.system(size: 11, weight: .medium, design: .monospaced))
        .foregroundStyle(.primary)
        .padding(.horizontal, 7)
        .padding(.vertical, 5)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 6))
        .overlay {
            RoundedRectangle(cornerRadius: 6)
                .stroke(.separator.opacity(0.6), lineWidth: 1)
        }
    }
}

private struct TerminalSearchPanel: View {
    @ObservedObject var terminal: TerminalViewModel
    @Binding var options: TerminalSearchOptions
    let focusRequest: Int
    let onFocusChanged: (Bool) -> Void
    let onClose: () -> Void

    @State private var query = ""
    @FocusState private var isFieldFocused: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)

            TextField("Search", text: $query)
                .textFieldStyle(.plain)
                .frame(width: 220)
                .focused($isFieldFocused)
                .onSubmit {
                    terminal.selectNextSearchMatch()
                }
                .onExitCommand {
                    onClose()
                }

            Text(matchSummary)
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .trailing)

            Button {
                terminal.selectPreviousSearchMatch()
            } label: {
                Image(systemName: "chevron.up")
            }
            .buttonStyle(.borderless)
            .disabled(terminal.searchMatches.isEmpty)

            Button {
                terminal.selectNextSearchMatch()
            } label: {
                Image(systemName: "chevron.down")
            }
            .buttonStyle(.borderless)
            .disabled(terminal.searchMatches.isEmpty)

            Button {
                options.caseSensitive.toggle()
            } label: {
                Text("Aa")
                    .font(.caption.weight(.semibold))
            }
            .buttonStyle(.borderless)
            .help("Case Sensitive")
            .foregroundStyle(options.caseSensitive ? Color.accentColor : Color.secondary)

            Button {
                options.usesRegex.toggle()
            } label: {
                Text(".*")
                    .font(.caption.monospaced().weight(.semibold))
            }
            .buttonStyle(.borderless)
            .help("Regex")
            .foregroundStyle(options.usesRegex ? Color.accentColor : Color.secondary)

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .stroke(.separator.opacity(0.8), lineWidth: 1)
        }
        .onAppear {
            focusSearchField()
            terminal.updateSearch(query, options: options)
        }
        .onChange(of: focusRequest) { _, _ in
            focusSearchField()
        }
        .onChange(of: isFieldFocused) { _, isFocused in
            onFocusChanged(isFocused)
        }
        .onChange(of: query) { _, value in
            terminal.updateSearch(value, options: options)
        }
        .onChange(of: options) { _, value in
            terminal.updateSearch(query, options: value)
        }
        .onDisappear {
            onFocusChanged(false)
        }
    }

    private var matchSummary: String {
        guard !query.isEmpty else {
            return "0/0"
        }
        guard !terminal.searchMatches.isEmpty else {
            return "0/0"
        }
        return "\(terminal.activeSearchMatchIndex + 1)/\(terminal.searchMatches.count)"
    }

    private func focusSearchField() {
        onFocusChanged(true)
        isFieldFocused = true
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 10_000_000)
            onFocusChanged(true)
            isFieldFocused = true
        }
    }
}
