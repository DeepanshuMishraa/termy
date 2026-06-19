import AppKit
import SwiftUI

private enum AppMetadata {
    static let displayName = "Termy"
    static let bundleIdentifier = "com.lassevestergaard.termy"
}

@MainActor
enum TermyNativeAppActions {
    static func openConfigFileInEditor() -> Bool {
        guard let configPath = TermyConfigurationStore.shared.configuration.configPath, !configPath.isEmpty else {
            return false
        }

        let url = URL(fileURLWithPath: configPath)
        do {
            let directory = url.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            if !FileManager.default.fileExists(atPath: url.path) {
                try "# Termy config\n".write(to: url, atomically: true, encoding: .utf8)
            }
            return NSWorkspace.shared.open(url)
        } catch {
            TermyErrorPresenter.present("Couldn't open the config file", error: error)
            return false
        }
    }

    static func prettifyConfig() -> Bool {
        do {
            try SettingsBridge.prettifyConfig()
            TermyConfigurationStore.shared.reload()
            NotificationCenter.default.post(name: .termySettingsChanged, object: nil)
            return true
        } catch {
            TermyErrorPresenter.present("Couldn't prettify the config file", error: error)
            return false
        }
    }

    static func showAppInfo() {
        NSApp.orderFrontStandardAboutPanel(nil)
    }

    static func installCLI() {
        do {
            let message = try SettingsBridge.installCLI()
            TermyToastCenter.shared.show(message, kind: .success)
        } catch {
            TermyErrorPresenter.present("Couldn't install the command line tool", error: error)
        }
    }

    static func restartApp() {
        let bundleURL = Bundle.main.bundleURL
        let configuration = NSWorkspace.OpenConfiguration()
        NSWorkspace.shared.openApplication(at: bundleURL, configuration: configuration) { _, _ in
            Task { @MainActor in
                NSApp.terminate(nil)
            }
        }
    }

    static func toggleNativeTabBarVisibility(for window: NSWindow?) -> Bool {
        NativeTabWindowManager.shared.showNativeTabBar(for: window)
    }
}

@main
struct TermySwiftApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @FocusedValue(\.terminalCommands) private var terminalCommands
    @StateObject private var configurationStore = TermyConfigurationStore.shared

    init() {
        // Runs the headless render benchmark and exits when `--benchmark` is
        // passed, before any window is created.
        TermyBenchmarkRunner.runIfRequested()
    }

    private var effectiveTerminalCommands: TerminalCommandSet? {
        terminalCommands ?? TerminalCommandRouter.shared.focusedCommandSet()
    }

    var body: some Scene {
        WindowGroup(AppMetadata.displayName) {
            TerminalWorkspaceView()
                .termyUIFont()
                .frame(minWidth: 760, minHeight: 480)
                .background(WindowConfigurator())
                .handlesSettingsOpenRequests()
                .onOpenURL { url in
                    TermyDeeplinkRouter.handle(url)
                }
        }
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Check for Updates…") {
                    Task { await AppUpdater.shared.checkForUpdates(userInitiated: true) }
                }
                Button("Install Command Line Tool…") {
                    TermyNativeAppActions.installCLI()
                }
            }

            CommandGroup(replacing: .appSettings) {
                OpenSettingsButton()
            }

            CommandGroup(replacing: .newItem) {
                Button("New Tab") {
                    if let effectiveTerminalCommands {
                        effectiveTerminalCommands.execute(.newTab)
                    } else {
                        NativeTabWindowManager.shared.openNativeTab()
                    }
                }
                .keyboardShortcut("t", modifiers: [.command])
            }

            CommandMenu("Terminal") {
                ForEach(1...9, id: \.self) { tabNumber in
                    Button("Select Tab \(tabNumber)") {
                        NativeTabWindowManager.shared.selectNativeTab(number: tabNumber)
                    }
                    .keyboardShortcut(KeyEquivalent(Character(String(tabNumber))), modifiers: [.command])
                }

                Divider()

                Button("Previous Tab") {
                    NativeTabWindowManager.shared.selectRelativeNativeTab(offset: -1)
                }

                Button("Next Tab") {
                    NativeTabWindowManager.shared.selectRelativeNativeTab(offset: 1)
                }

                Button("Move Tab Left") {
                    NativeTabWindowManager.shared.moveSelectedNativeTab(offset: -1)
                }

                Button("Move Tab Right") {
                    NativeTabWindowManager.shared.moveSelectedNativeTab(offset: 1)
                }

                Divider()

                Button("Split Right") {
                    if !TerminalCommandRouter.shared.splitFocused(.horizontal) {
                        effectiveTerminalCommands?.execute(.splitPaneVertical)
                    }
                }
                .keyboardShortcut("d", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Split Down") {
                    if !TerminalCommandRouter.shared.splitFocused(.vertical) {
                        effectiveTerminalCommands?.execute(.splitPaneHorizontal)
                    }
                }
                .keyboardShortcut("d", modifiers: [.command, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Divider()

                Button("Close Pane or Tab") {
                    effectiveTerminalCommands?.execute(.closePaneOrTab)
                }
                .keyboardShortcut("w", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Close Pane") {
                    effectiveTerminalCommands?.execute(.closePane)
                }
                .keyboardShortcut("w", modifiers: [.command, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Divider()

                Button("Next Pane") {
                    effectiveTerminalCommands?.execute(.focusPaneNext)
                }
                .keyboardShortcut("o", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Previous Pane") {
                    effectiveTerminalCommands?.execute(.focusPanePrevious)
                }
                .keyboardShortcut("o", modifiers: [.command, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Button("Focus Pane Left") {
                    effectiveTerminalCommands?.execute(.focusPane(.left))
                }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
                .disabled(effectiveTerminalCommands == nil)

                Button("Focus Pane Right") {
                    effectiveTerminalCommands?.execute(.focusPane(.right))
                }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
                .disabled(effectiveTerminalCommands == nil)

                Button("Focus Pane Up") {
                    effectiveTerminalCommands?.execute(.focusPane(.up))
                }
                .keyboardShortcut(.upArrow, modifiers: [.command, .option])
                .disabled(effectiveTerminalCommands == nil)

                Button("Focus Pane Down") {
                    effectiveTerminalCommands?.execute(.focusPane(.down))
                }
                .keyboardShortcut(.downArrow, modifiers: [.command, .option])
                .disabled(effectiveTerminalCommands == nil)

                Divider()

                Button("Resize Pane Left") {
                    effectiveTerminalCommands?.execute(.resizePane(.left))
                }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .option, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Button("Resize Pane Right") {
                    effectiveTerminalCommands?.execute(.resizePane(.right))
                }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .option, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Button("Resize Pane Up") {
                    effectiveTerminalCommands?.execute(.resizePane(.up))
                }
                .keyboardShortcut(.upArrow, modifiers: [.command, .option, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Button("Resize Pane Down") {
                    effectiveTerminalCommands?.execute(.resizePane(.down))
                }
                .keyboardShortcut(.downArrow, modifiers: [.command, .option, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Button("Toggle Pane Zoom") {
                    effectiveTerminalCommands?.execute(.togglePaneZoom)
                }
                .keyboardShortcut(.return, modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Divider()

                Button("Increase Font Size") {
                    effectiveTerminalCommands?.execute(.increaseFontSize)
                }
                .keyboardShortcut("=", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Decrease Font Size") {
                    effectiveTerminalCommands?.execute(.decreaseFontSize)
                }
                .keyboardShortcut("-", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Reset Font Size") {
                    effectiveTerminalCommands?.execute(.resetFontSize)
                }
                .keyboardShortcut("0", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Divider()

                if !configurationStore.configuration.tasks.isEmpty {
                    Menu("Tasks") {
                        ForEach(configurationStore.configuration.tasks) { task in
                            Button(task.name) {
                                NativeTabWindowManager.shared.openNativeTab(startupTask: task)
                            }
                        }
                    }

                    Divider()
                }

                Button("Send Interrupt") {
                    effectiveTerminalCommands?.execute(.sendInterrupt)
                }
                .keyboardShortcut("c", modifiers: [.control])
                .disabled(effectiveTerminalCommands == nil)
            }

            CommandGroup(after: .textEditing) {
                Button("Find") {
                    effectiveTerminalCommands?.execute(.openSearch)
                }
                .keyboardShortcut("f", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Find Next") {
                    effectiveTerminalCommands?.execute(.searchNext)
                }
                .keyboardShortcut("g", modifiers: [.command])
                .disabled(effectiveTerminalCommands == nil)

                Button("Find Previous") {
                    effectiveTerminalCommands?.execute(.searchPrevious)
                }
                .keyboardShortcut("g", modifiers: [.command, .shift])
                .disabled(effectiveTerminalCommands == nil)

                Button("Case Sensitive") {
                    effectiveTerminalCommands?.execute(.toggleSearchCaseSensitive)
                }
                .keyboardShortcut("c", modifiers: [.command, .option])
                .disabled(effectiveTerminalCommands == nil)

                Button("Regex") {
                    effectiveTerminalCommands?.execute(.toggleSearchRegex)
                }
                .keyboardShortcut("r", modifiers: [.command, .option])
                .disabled(effectiveTerminalCommands == nil)

                Button("Close Search") {
                    effectiveTerminalCommands?.execute(.closeSearch)
                }
                .keyboardShortcut(.escape, modifiers: [])
                .disabled(effectiveTerminalCommands == nil)
            }
        }

        Window("\(AppMetadata.displayName) Settings", id: Self.settingsWindowID) {
            SettingsRootView()
                .termyUIFont()
        }
        .defaultSize(width: 860, height: 600)
        .windowResizability(.contentMinSize)
    }

    static let settingsWindowID = "termy-settings"
}

/// Opens settings in a dedicated window while preserving the standard shortcut.
private struct OpenSettingsButton: View {
    @Environment(\.openWindow) private var openWindow
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared

    var body: some View {
        Button("Settings…") {
            if configurationStore.configuration.native.simpleMode,
               TermyNativeAppActions.openConfigFileInEditor() {
                NSApp.activate(ignoringOtherApps: true)
            } else {
                openWindow(id: TermySwiftApp.settingsWindowID)
                NSApp.activate(ignoringOtherApps: true)
            }
        }
        .termyUIFont()
        .keyboardShortcut(",", modifiers: .command)
    }
}

private struct SettingsOpenRequestHandler: ViewModifier {
    @Environment(\.openWindow) private var openWindow
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared

    func body(content: Content) -> some View {
        content.onReceive(NotificationCenter.default.publisher(for: .termyOpenSettingsRequested)) { _ in
            openSettings()
        }
    }

    private func openSettings() {
        if configurationStore.configuration.native.simpleMode,
           TermyNativeAppActions.openConfigFileInEditor() {
            NSApp.activate(ignoringOtherApps: true)
        } else {
            openWindow(id: TermySwiftApp.settingsWindowID)
            NSApp.activate(ignoringOtherApps: true)
        }
    }
}

private extension View {
    func handlesSettingsOpenRequests() -> some View {
        modifier(SettingsOpenRequestHandler())
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var closePaneEventMonitor: LocalEventMonitor?
    private var settingsObserver: NSObjectProtocol?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSWindow.allowsAutomaticWindowTabbing = true
        AppLogoManager.shared.applyToDock()
        OnboardingPresenter.shared.presentIfNeeded()
        if TermyConfigurationStore.shared.configuration.native.autoUpdate {
            Task { await AppUpdater.shared.checkForUpdates(userInitiated: false) }
        }
        settingsObserver = NotificationCenter.default.addObserver(
            forName: .termySettingsChanged,
            object: nil,
            queue: .main
        ) { _ in
            Task { @MainActor in
                AppLogoManager.shared.reloadFromConfig()
            }
        }
        if let monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown, handler: { event in
            if ConfiguredKeybindRouter.shared.handle(event) {
                return nil
            }

            if Self.handleDefaultFontZoomShortcut(event) {
                return nil
            }

            guard event.modifierFlags.contains(.command),
                  !event.modifierFlags.contains(.shift),
                  event.charactersIgnoringModifiers?.lowercased() == "w"
            else {
                return event
            }

            return TerminalCommandRouter.shared.closeFocusedPaneIfSplit(for: event) ? nil : event
        }) {
            closePaneEventMonitor = LocalEventMonitor(monitor)
        }
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationWillTerminate(_ notification: Notification) {
        closePaneEventMonitor?.invalidate()
        if let settingsObserver {
            NotificationCenter.default.removeObserver(settingsObserver)
            self.settingsObserver = nil
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        let safety = TermySafetyConfiguration.loadCurrent()
        let hasRunningProcess = TerminalCommandRouter.shared.hasRunningTerminalProcess()
        guard safety.warnOnQuit || (safety.warnOnQuitWithRunningProcess && hasRunningProcess) else {
            return .terminateNow
        }

        let alert = NSAlert()
        alert.messageText = hasRunningProcess ? "Quit Termy with running processes?" : "Quit Termy?"
        alert.informativeText = hasRunningProcess
            ? "One or more terminal panes still have a running process."
            : "The safety setting requires confirmation before quitting."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Quit")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn ? .terminateNow : .terminateCancel
    }

    @objc func newWindowForTab(_ sender: Any?) {
        NativeTabWindowManager.shared.openNativeTab()
    }

    private static func handleDefaultFontZoomShortcut(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags
        guard flags.contains(.command),
              !flags.contains(.control),
              !flags.contains(.option)
        else {
            return false
        }

        guard let terminal = TerminalCommandRouter.shared.focusedStore(for: event)?.focusedTerminal else {
            return false
        }

        switch fontZoomShortcut(for: event) {
        case .increase:
            terminal.increaseFontSize()
        case .decrease:
            terminal.decreaseFontSize()
        case .reset:
            terminal.resetFontSize()
        case nil:
            return false
        }
        return true
    }

    private enum FontZoomShortcut {
        case increase
        case decrease
        case reset
    }

    private static func fontZoomShortcut(for event: NSEvent) -> FontZoomShortcut? {
        let characters = [
            event.characters,
            event.charactersIgnoringModifiers
        ].compactMap { $0?.lowercased() }

        if characters.contains("+") || characters.contains("=") {
            return .increase
        }
        if characters.contains("-") {
            return .decrease
        }
        if characters.contains("0") {
            return .reset
        }

        switch event.keyCode {
        case 24, 69:
            return .increase
        case 27, 78:
            return .decrease
        case 29, 82:
            return .reset
        default:
            return nil
        }
    }
}

@MainActor
private final class ConfiguredKeybindRouter {
    static let shared = ConfiguredKeybindRouter()

    private var configuration = TermyConfigurationStore.shared.configuration
    private var settingsObserver: NSObjectProtocol?

    private init() {
        settingsObserver = NotificationCenter.default.addObserver(
            forName: .termySettingsChanged,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.configuration = TermyConfigurationStore.shared.reload()
            }
        }
    }

    func handle(_ event: NSEvent) -> Bool {
        let triggers = canonicalTriggers(for: event)
        guard !triggers.isEmpty,
              let keybind = configuration.keybinds.first(where: { triggers.contains($0.trigger) })
        else {
            return false
        }

        return execute(keybind.keybindAction, event: event)
    }

    /// Runs `body` against the window's focused store, reporting whether a store
    /// was present (i.e. whether the keybind was handled).
    private func withFocusedStore(_ event: NSEvent, _ body: (TerminalWorkspaceStore) -> Void) -> Bool {
        guard let store = TerminalCommandRouter.shared.focusedStore(for: event) else {
            return false
        }
        body(store)
        return true
    }

    private func execute(_ action: TerminalKeybindAction, event: NSEvent) -> Bool {
        switch action {
        case .appInfo:
            TermyNativeAppActions.showAppInfo()
            return true
        case .restartApp:
            TermyNativeAppActions.restartApp()
            return true
        case .openConfig:
            return TermyNativeAppActions.openConfigFileInEditor()
        case .prettifyConfig:
            return TermyNativeAppActions.prettifyConfig()
        case .toggleTabBarVisibility:
            return TermyNativeAppActions.toggleNativeTabBarVisibility(for: event.window)
        case .moveTabLeft:
            NativeTabWindowManager.shared.moveSelectedNativeTab(offset: -1)
            return true
        case .moveTabRight:
            NativeTabWindowManager.shared.moveSelectedNativeTab(offset: 1)
            return true
        case .switchTabLeft:
            NativeTabWindowManager.shared.selectRelativeNativeTab(offset: -1)
            return true
        case .switchTabRight:
            NativeTabWindowManager.shared.selectRelativeNativeTab(offset: 1)
            return true
        case .toggleCommandPalette:
            guard let store = TerminalCommandRouter.shared.focusedStore(for: event),
                  !configuration.native.simpleMode
            else {
                return false
            }
            store.toggleCommandPalette()
            return true
        case .newTab:
            NativeTabWindowManager.shared.openNativeTab()
            return true
        case .closeTab:
            (event.window ?? NSApp.keyWindow)?.performClose(nil)
            return true
        case .closePaneOrTab:
            if TerminalCommandRouter.shared.closeFocusedPaneIfSplit(for: event) {
                return true
            }
            (event.window ?? NSApp.keyWindow)?.performClose(nil)
            return true
        case .closePane:
            return withFocusedStore(event) { $0.closeFocusedPane() }
        case .splitPaneVertical:
            return TerminalCommandRouter.shared.splitFocused(.horizontal, for: event.window)
        case .splitPaneHorizontal:
            return TerminalCommandRouter.shared.splitFocused(.vertical, for: event.window)
        case .focusPaneNext:
            return withFocusedStore(event) { $0.focusNextPane() }
        case .focusPanePrevious:
            return withFocusedStore(event) { $0.focusPreviousPane() }
        case .focusPane(let direction):
            return TerminalCommandRouter.shared.focusedStore(for: event)?.focusPane(in: direction) ?? false
        case .resizePane(let direction):
            return TerminalCommandRouter.shared.focusedStore(for: event)?.resizeFocusedPane(in: direction) ?? false
        case .togglePaneZoom:
            return withFocusedStore(event) { $0.toggleFocusedPaneZoom() }
        case .increaseFontSize:
            return withFocusedStore(event) { $0.focusedTerminal?.increaseFontSize() }
        case .decreaseFontSize:
            return withFocusedStore(event) { $0.focusedTerminal?.decreaseFontSize() }
        case .resetFontSize:
            return withFocusedStore(event) { $0.focusedTerminal?.resetFontSize() }
        case .copy:
            return TerminalCommandRouter.shared.focusedStore(for: event)?.focusedTerminal?.copySelection() ?? false
        case .paste:
            guard let text = NSPasteboard.general.string(forType: .string) else {
                return false
            }
            TerminalCommandRouter.shared.focusedStore(for: event)?.focusedTerminal?.paste(text)
            return true
        case .openSearch:
            return withFocusedStore(event) { $0.showSearch() }
        case .closeSearch:
            return withFocusedStore(event) { $0.hideSearch() }
        case .searchNext:
            TerminalCommandRouter.shared.focusedStore(for: event)?.focusedTerminal?.selectNextSearchMatch()
            return true
        case .searchPrevious:
            TerminalCommandRouter.shared.focusedStore(for: event)?.focusedTerminal?.selectPreviousSearchMatch()
            return true
        case .toggleSearchCaseSensitive:
            TerminalCommandRouter.shared.focusedStore(for: event)?.toggleSearchCaseSensitive()
            return true
        case .toggleSearchRegex:
            TerminalCommandRouter.shared.focusedStore(for: event)?.toggleSearchRegex()
            return true
        case .switchToTab(let number):
            NativeTabWindowManager.shared.selectNativeTab(number: number)
            return true
        case .minimizeWindow:
            (event.window ?? NSApp.keyWindow)?.miniaturize(nil)
            return true
        case .quit:
            NSApp.terminate(nil)
            return true
        case .clearScrollback, .sendInterrupt, .runTask, .unknown:
            // Not bound as keybinds (palette-only or task payload required).
            return false
        }
    }

    private func canonicalTriggers(for event: NSEvent) -> Set<String> {
        guard let key = keyName(for: event) else {
            return []
        }

        let flags = event.modifierFlags
        var baseModifiers: [String] = []
        if flags.contains(.control) {
            baseModifiers.append("ctrl")
        }
        if flags.contains(.option) {
            baseModifiers.append("alt")
        }
        if flags.contains(.shift) {
            baseModifiers.append("shift")
        }

        var triggers = Set<String>()
        if flags.contains(.command) {
            triggers.insert((baseModifiers + ["cmd", key]).joined(separator: "-"))
            triggers.insert((baseModifiers + ["secondary", key]).joined(separator: "-"))
        } else {
            triggers.insert((baseModifiers + [key]).joined(separator: "-"))
        }
        return triggers
    }

    private func keyName(for event: NSEvent) -> String? {
        switch event.keyCode {
        case 36, 76:
            return "enter"
        case 48:
            return "tab"
        case 53:
            return "escape"
        case 49:
            return "space"
        case 123:
            return "left"
        case 124:
            return "right"
        case 125:
            return "down"
        case 126:
            return "up"
        default:
            guard let characters = event.charactersIgnoringModifiers?.lowercased(),
                  let scalar = characters.unicodeScalars.first
            else {
                return nil
            }
            return String(scalar)
        }
    }
}

private final class LocalEventMonitor {
    private var invalidateHandler: (() -> Void)?

    init<Token>(_ token: Token) {
        invalidateHandler = {
            NSEvent.removeMonitor(token)
        }
    }

    func invalidate() {
        invalidateHandler?()
        invalidateHandler = nil
    }

    deinit {
        invalidate()
    }
}

struct WindowConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            if let window = view.window {
                NativeTabWindowManager.shared.configure(window)
            }
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            if let window = view.window {
                NativeTabWindowManager.shared.configure(window)
            }
        }
    }
}

@MainActor
struct NativeTabDescriptor: Identifiable {
    var id: ObjectIdentifier
    var index: Int
    var title: String
    var isSelected: Bool
    var isPinned: Bool
    var hasManualTitle: Bool
    fileprivate weak var window: NSWindow?
}

@MainActor
final class NativeTabWindowManager: NSObject, NSWindowDelegate {
    static let shared = NativeTabWindowManager()

    private var retainedWindows: [NSWindow] = []
    private var configuredWindowIDs = Set<ObjectIdentifier>()
    private let tabbingIdentifier = "\(AppMetadata.bundleIdentifier).native-tabs"
    private var entranceTabID: ObjectIdentifier?
    private var entranceIncludesBar = false
    private var entranceDeadline = Date.distantPast

    func configure(_ window: NSWindow) {
        window.tabbingMode = .preferred
        window.tabbingIdentifier = tabbingIdentifier
        window.collectionBehavior.insert(.fullScreenPrimary)

        let identifier = ObjectIdentifier(window)
        guard !configuredWindowIDs.contains(identifier) else {
            showSystemTabBarIfNeeded(for: window)
            applyFocusedTerminalChrome(for: window)
            return
        }
        configuredWindowIDs.insert(identifier)
        if window.title.isEmpty || window.title == "Window" {
            window.title = AppMetadata.displayName
        }
        TerminalWindowChromeApplier.applyFocusedChrome(
            TerminalWindowChromeState(
                title: window.title,
                isFocused: true,
                background: TerminalRenderConfig.default.background,
                backgroundOpacity: TerminalRenderConfig.default.backgroundOpacity,
                backgroundBlur: TerminalRenderConfig.default.backgroundBlur
            ),
            to: window
        )
        showSystemTabBarIfNeeded(for: window)
        window.setContentSize(TermyConfigurationStore.shared.configuration.windowSize)
        window.center()
        postTabsChanged()
    }

    func openNativeTab(startupTask: TermyTaskConfiguration? = nil) {
        let window = makeWindow(startupTask: startupTask)
        retainedWindows.append(window)

        let anchorWindow = NSApp.keyWindow ?? NSApp.mainWindow
        noteTabEntrance(for: window, anchorWindow: anchorWindow)
        if let currentWindow = anchorWindow {
            configure(currentWindow)
            currentWindow.addTabbedWindow(window, ordered: .above)
        }

        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        showSystemTabBarIfNeeded(for: window)
        applyFocusedTerminalChrome(for: window)
        postTabsChanged()
    }

    /// Marks a freshly opened tab so the chrome can play its entrance
    /// animation. Each native tab is its own window with its own chrome view,
    /// so opening a tab swaps to a freshly mounted chrome where a plain
    /// SwiftUI insertion transition never fires — the marker travels with the
    /// tab instead and expires shortly after the open.
    func noteTabEntrance(for window: NSWindow, anchorWindow: NSWindow?) {
        entranceTabID = ObjectIdentifier(window)
        let previousTabCount = anchorWindow.map { nativeTabWindows(for: $0).count } ?? 0
        let autoHideTabbar = TermyConfigurationStore.shared.configuration.native.autoHideTabbar
        entranceIncludesBar = autoHideTabbar && previousTabCount <= 1
        entranceDeadline = Date().addingTimeInterval(0.8)
    }

    /// Whether `tabID` was just opened and its chrome should animate it in.
    func shouldAnimateTabEntrance(for tabID: ObjectIdentifier) -> Bool {
        tabID == entranceTabID && Date() < entranceDeadline
    }

    /// Whether the whole tab bar should slide in on `window`: only when the
    /// just-opened tab made the auto-hidden bar visible for the first time.
    func shouldAnimateBarEntrance(for window: NSWindow?) -> Bool {
        guard let window else {
            return false
        }
        return entranceIncludesBar
            && ObjectIdentifier(window) == entranceTabID
            && Date() < entranceDeadline
    }

    func tabDescriptors(for sourceWindow: NSWindow?) -> [NativeTabDescriptor] {
        let sourceWindow = sourceWindow.flatMap { isNativeTerminalTabWindow($0) ? $0 : nil }
            ?? nativeTabSourceWindow()
        guard let sourceWindow else {
            return []
        }

        let selectedWindow = NSApp.keyWindow ?? NSApp.mainWindow
        return nativeTabWindows(for: sourceWindow).enumerated().map { index, window in
            let store = TerminalCommandRouter.shared.store(forWindow: window)
            let trimmedTitle = (store?.tabDisplayTitle ?? window.title).trimmingCharacters(in: .whitespacesAndNewlines)
            return NativeTabDescriptor(
                id: ObjectIdentifier(window),
                index: index,
                title: trimmedTitle.isEmpty ? AppMetadata.displayName : trimmedTitle,
                isSelected: window === selectedWindow,
                isPinned: store?.tabPinned ?? false,
                hasManualTitle: store?.tabManualTitle != nil,
                window: window
            )
        }
    }

    func selectNativeTab(_ descriptor: NativeTabDescriptor) {
        guard let window = descriptor.window else {
            return
        }
        if window.isMiniaturized {
            window.deminiaturize(nil)
        }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        showSystemTabBarIfNeeded(for: window)
        applyFocusedTerminalChrome(for: window)
        postTabsChanged()
    }

    func closeNativeTab(_ descriptor: NativeTabDescriptor) {
        descriptor.window?.performClose(nil)
        postTabsChanged()
    }

    func setNativeTabPinned(_ descriptor: NativeTabDescriptor, pinned: Bool) {
        guard let window = descriptor.window,
              let store = TerminalCommandRouter.shared.store(forWindow: window)
        else {
            return
        }
        store.setTabPinned(pinned)
        postTabsChanged()
    }

    func renameNativeTab(_ descriptor: NativeTabDescriptor, title: String) {
        guard let window = descriptor.window,
              let store = TerminalCommandRouter.shared.store(forWindow: window)
        else {
            return
        }
        store.renameTab(title)
        window.title = store.tabDisplayTitle
        postTabsChanged()
    }

    /// Brings a tabbed window to the front, restoring it if miniaturized.
    private func activateTabbedWindow(_ window: NSWindow) {
        if window.isMiniaturized {
            window.deminiaturize(nil)
        }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        showSystemTabBarIfNeeded(for: window)
        applyFocusedTerminalChrome(for: window)
        postTabsChanged()
    }

    func selectNativeTab(number: Int) {
        let index = number - 1
        guard index >= 0,
              let sourceWindow = nativeTabSourceWindow()
        else {
            return
        }

        let tabbedWindows = nativeTabWindows(for: sourceWindow)
        guard tabbedWindows.indices.contains(index) else {
            return
        }

        activateTabbedWindow(tabbedWindows[index])
    }

    func selectRelativeNativeTab(offset: Int) {
        guard offset != 0,
              let sourceWindow = nativeTabSourceWindow()
        else {
            return
        }

        let tabbedWindows = nativeTabWindows(for: sourceWindow)
        guard !tabbedWindows.isEmpty else {
            return
        }

        let selectedWindow = NSApp.keyWindow ?? NSApp.mainWindow
        let currentIndex = tabbedWindows.firstIndex { $0 === selectedWindow } ?? 0
        let targetIndex = (currentIndex + offset + tabbedWindows.count) % tabbedWindows.count
        activateTabbedWindow(tabbedWindows[targetIndex])
    }

    func moveSelectedNativeTab(offset: Int) {
        guard offset != 0,
              let sourceWindow = nativeTabSourceWindow()
        else {
            return
        }

        let tabbedWindows = nativeTabWindows(for: sourceWindow)
        guard tabbedWindows.count > 1 else {
            return
        }

        let selectedWindow = NSApp.keyWindow ?? NSApp.mainWindow
        guard let currentIndex = tabbedWindows.firstIndex(where: { $0 === selectedWindow }) else {
            return
        }
        let targetIndex = max(0, min(tabbedWindows.count - 1, currentIndex + offset))
        guard targetIndex != currentIndex else {
            return
        }

        let movingWindow = tabbedWindows[currentIndex]
        let anchorWindow = tabbedWindows[targetIndex]
        anchorWindow.addTabbedWindow(movingWindow, ordered: offset < 0 ? .below : .above)
        movingWindow.makeKeyAndOrderFront(nil)
        showSystemTabBarIfNeeded(for: movingWindow)
        applyFocusedTerminalChrome(for: movingWindow)
        postTabsChanged()
    }

    /// Moves a specific tab to `targetIndex`, used by drag-to-reorder in the
    /// custom tab chrome.
    func moveNativeTab(_ descriptor: NativeTabDescriptor, toIndex targetIndex: Int) {
        guard let movingWindow = descriptor.window,
              let sourceWindow = nativeTabSourceWindow()
        else {
            return
        }

        let tabbedWindows = nativeTabWindows(for: sourceWindow)
        guard let currentIndex = tabbedWindows.firstIndex(where: { $0 === movingWindow }) else {
            return
        }
        let clamped = max(0, min(tabbedWindows.count - 1, targetIndex))
        guard clamped != currentIndex else {
            return
        }

        let anchorWindow = tabbedWindows[clamped]
        anchorWindow.addTabbedWindow(movingWindow, ordered: clamped < currentIndex ? .below : .above)
        movingWindow.makeKeyAndOrderFront(nil)
        showSystemTabBarIfNeeded(for: movingWindow)
        applyFocusedTerminalChrome(for: movingWindow)
        postTabsChanged()
    }

    func showNativeTabBar(for window: NSWindow?) -> Bool {
        guard let window = window ?? NSApp.keyWindow ?? NSApp.mainWindow,
              isNativeTerminalTabWindow(window)
        else {
            return false
        }
        showSystemTabBarIfNeeded(for: window)
        applyFocusedTerminalChrome(for: window)
        return true
    }

    func windowWillClose(_ notification: Notification) {
        guard let window = notification.object as? NSWindow else {
            return
        }
        retainedWindows.removeAll { $0 === window }
        configuredWindowIDs.remove(ObjectIdentifier(window))
        postTabsChanged()
    }

    func windowDidBecomeKey(_ notification: Notification) {
        if let window = notification.object as? NSWindow {
            showSystemTabBarIfNeeded(for: window)
            applyFocusedTerminalChrome(for: window)
        }
        postTabsChanged()
    }

    /// Suspend refresh polling for a window's terminals while it is fully
    /// occluded (e.g. a background native tab), and resume when it becomes
    /// visible again. Keeps occluded tabs from competing for the main run loop.
    func windowDidChangeOcclusionState(_ notification: Notification) {
        guard let window = notification.object as? NSWindow,
              let store = TerminalCommandRouter.shared.store(forWindow: window)
        else {
            return
        }
        if window.occlusionState.contains(.visible) {
            store.resumeRefresh()
        } else {
            store.suspendRefresh()
        }
    }

    private func makeWindow(startupTask: TermyTaskConfiguration? = nil) -> NSWindow {
        let windowSize = TermyConfigurationStore.shared.configuration.windowSize
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: windowSize.width, height: windowSize.height),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.center()
        window.contentViewController = NSHostingController(rootView: TerminalWorkspaceView(initialTask: startupTask))
        window.isReleasedWhenClosed = false
        window.delegate = self
        configure(window)
        return window
    }

    private func nativeTabSourceWindow() -> NSWindow? {
        for window in [NSApp.keyWindow, NSApp.mainWindow].compactMap(\.self) {
            if isNativeTerminalTabWindow(window) {
                return window
            }
        }

        return NSApp.windows.first(where: isNativeTerminalTabWindow)
    }

    private func nativeTabWindows(for sourceWindow: NSWindow) -> [NSWindow] {
        let windows = sourceWindow.tabbedWindows ?? [sourceWindow]
        return windows.filter(isNativeTerminalTabWindow)
    }

    private func isNativeTerminalTabWindow(_ window: NSWindow) -> Bool {
        window.tabbingIdentifier == tabbingIdentifier
    }

    private func showSystemTabBarIfNeeded(for window: NSWindow) {
        guard isNativeTerminalTabWindow(window),
              nativeTabWindows(for: window).count > 1,
              window.tabGroup?.isTabBarVisible == false
        else {
            return
        }
        window.toggleTabBar(nil)
    }

    @discardableResult
    func applyFocusedTerminalChrome(for window: NSWindow?) -> Bool {
        guard let window,
              isNativeTerminalTabWindow(window),
              let store = TerminalCommandRouter.shared.store(forWindow: window),
              let renderConfig = store.focusedTerminal?.renderConfig
        else {
            return false
        }

        let titleChanged = TerminalWindowChromeApplier.applyFocusedChrome(
            TerminalWindowChromeState(
                title: store.tabDisplayTitle,
                isFocused: true,
                background: renderConfig.background,
                backgroundOpacity: renderConfig.backgroundOpacity,
                backgroundBlur: renderConfig.backgroundBlur
            ),
            to: window
        )
        if titleChanged {
            postTabsChanged()
        }
        return true
    }

    private func postTabsChanged() {
        NotificationCenter.default.post(name: .termyNativeTabsChanged, object: nil)
    }
}
