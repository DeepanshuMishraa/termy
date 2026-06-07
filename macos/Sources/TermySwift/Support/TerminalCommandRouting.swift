import AppKit

enum TerminalHostCommand {
    case newTab
    case closePaneOrTab
    case splitPaneVertical
    case splitPaneHorizontal
    case closePane
    case focusPane(TerminalPaneDirection)
    case focusPaneNext
    case focusPanePrevious
    case resizePane(TerminalPaneDirection)
    case togglePaneZoom
    case increaseFontSize
    case decreaseFontSize
    case resetFontSize
    case copy
    case paste
    case openSearch
    case closeSearch
    case searchNext
    case searchPrevious
    case toggleSearchCaseSensitive
    case toggleSearchRegex
    case clearScrollback
    case sendInterrupt
    case toggleCommandPalette
}

@MainActor
final class TerminalCommandRouter {
    static let shared = TerminalCommandRouter()

    weak var activeStore: TerminalWorkspaceStore?
    private var storesByWindow: [ObjectIdentifier: WeakTerminalWorkspaceStore] = [:]

    func activate(_ store: TerminalWorkspaceStore) {
        activeStore = store
    }

    func register(_ store: TerminalWorkspaceStore, for window: NSWindow) {
        storesByWindow[ObjectIdentifier(window)] = WeakTerminalWorkspaceStore(store)
        activate(store)
    }

    func unregister(window: NSWindow) {
        storesByWindow.removeValue(forKey: ObjectIdentifier(window))
    }

    /// The store hosted by a specific window, with no active-store fallback.
    /// Used to suspend/resume a window's panes on occlusion changes.
    func store(forWindow window: NSWindow) -> TerminalWorkspaceStore? {
        storesByWindow[ObjectIdentifier(window)]?.store
    }

    func closeFocusedPaneIfSplit(for event: NSEvent? = nil) -> Bool {
        store(for: event?.window ?? NSApp.keyWindow ?? NSApp.mainWindow)?
            .closeFocusedPaneIfSplit() ?? false
    }

    func splitFocused(_ axis: TerminalSplitAxis, for window: NSWindow? = nil) -> Bool {
        guard let store = store(for: window ?? NSApp.keyWindow ?? NSApp.mainWindow) else {
            return false
        }

        store.splitFocused(axis)
        return true
    }

    func focusedStore(for event: NSEvent? = nil) -> TerminalWorkspaceStore? {
        store(for: event?.window ?? NSApp.keyWindow ?? NSApp.mainWindow)
    }

    func hasRunningTerminalProcess() -> Bool {
        cleanupReleasedStores()
        if activeStore?.hasRunningTerminalProcess == true {
            return true
        }
        return storesByWindow.values.contains { $0.store?.hasRunningTerminalProcess == true }
    }

    private func store(for window: NSWindow?) -> TerminalWorkspaceStore? {
        cleanupReleasedStores()
        guard let window else {
            return activeStore
        }
        return storesByWindow[ObjectIdentifier(window)]?.store ?? activeStore
    }

    private func cleanupReleasedStores() {
        storesByWindow = storesByWindow.filter { _, box in
            box.store != nil
        }
    }
}

private final class WeakTerminalWorkspaceStore {
    weak var store: TerminalWorkspaceStore?

    init(_ store: TerminalWorkspaceStore) {
        self.store = store
    }
}

struct TerminalCommandSet {
    var newTab: () -> Void = {}
    var closePaneOrTab: () -> Void = {}
    var splitRight: () -> Void
    var splitDown: () -> Void
    var closePane: () -> Void
    var focusPane: (TerminalPaneDirection) -> Void = { _ in }
    var focusNextPane: () -> Void
    var focusPreviousPane: () -> Void = {}
    var resizePane: (TerminalPaneDirection) -> Void = { _ in }
    var togglePaneZoom: () -> Void = {}
    var increaseFontSize: () -> Void = {}
    var decreaseFontSize: () -> Void = {}
    var resetFontSize: () -> Void = {}
    var copy: () -> Bool = { false }
    var paste: () -> Void = {}
    var clearScrollback: () -> Void = {}
    var showSearch: () -> Void
    var hideSearch: () -> Void
    var searchNext: () -> Void = {}
    var searchPrevious: () -> Void = {}
    var toggleSearchCaseSensitive: () -> Void = {}
    var toggleSearchRegex: () -> Void = {}
    var sendInterrupt: () -> Void
    var toggleCommandPalette: () -> Void = {}

    func execute(_ command: TerminalHostCommand) {
        switch command {
        case .newTab:
            newTab()
        case .closePaneOrTab:
            closePaneOrTab()
        case .splitPaneVertical:
            splitRight()
        case .splitPaneHorizontal:
            splitDown()
        case .closePane:
            closePane()
        case .focusPane(let direction):
            focusPane(direction)
        case .focusPaneNext:
            focusNextPane()
        case .focusPanePrevious:
            focusPreviousPane()
        case .resizePane(let direction):
            resizePane(direction)
        case .togglePaneZoom:
            togglePaneZoom()
        case .increaseFontSize:
            increaseFontSize()
        case .decreaseFontSize:
            decreaseFontSize()
        case .resetFontSize:
            resetFontSize()
        case .copy:
            _ = copy()
        case .paste:
            paste()
        case .openSearch:
            showSearch()
        case .closeSearch:
            hideSearch()
        case .searchNext:
            searchNext()
        case .searchPrevious:
            searchPrevious()
        case .toggleSearchCaseSensitive:
            toggleSearchCaseSensitive()
        case .toggleSearchRegex:
            toggleSearchRegex()
        case .clearScrollback:
            clearScrollback()
        case .sendInterrupt:
            sendInterrupt()
        case .toggleCommandPalette:
            toggleCommandPalette()
        }
    }
}
