import Foundation

@MainActor
final class TmuxControlWorkspaceModel: ObservableObject {
    @Published private(set) var layout: TmuxLayoutNode?
    @Published private(set) var errorMessage: String?
    @Published var focusedPaneID: Int?
    @Published var isSearchVisible = false
    @Published private(set) var isSearchInputFocused = false
    @Published private(set) var searchFocusRequest = 0
    @Published var searchOptions = TerminalSearchOptions()

    private let session: TmuxControlSession
    private var terminals: [Int: TerminalViewModel] = [:]
    private var pumpTask: Task<Void, Never>?

    init(configuration: TermyAppConfiguration = TermyConfigurationStore.shared.configuration) throws {
        guard configuration.tmux.enabled,
              let binary = TmuxIntegration.tmuxBinaryPath(for: configuration.tmux)
        else {
            throw TmuxControlWorkspaceError.disabled
        }

        let socket = Self.uniqueTmuxSocketName()
        let sessionName = Self.uniqueTmuxSessionName()
        session = try TmuxControlSession(
            binary: binary,
            socket: socket,
            session: sessionName,
            loadUserConfig: true
        )
    }

    init(session: TmuxControlSession) {
        self.session = session
    }

    var focusedTerminal: TerminalViewModel? {
        focusedPaneID.flatMap { terminals[$0] }
    }

    var paneCount: Int {
        terminals.count
    }

    var tabDisplayTitle: String {
        guard let focusedPaneID else {
            return "tmux"
        }
        return "tmux %\(focusedPaneID)"
    }

    func terminal(forPane id: Int) -> TerminalViewModel? {
        terminals[id]
    }

    @discardableResult
    func start() -> Bool {
        guard pumpTask == nil else {
            return layout != nil && !terminals.isEmpty
        }
        guard reconcileLayout() else {
            return false
        }
        pumpTask = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                self?.pump()
                try? await Task.sleep(nanoseconds: 33_000_000)
            }
        }
        return true
    }

    func stop() {
        pumpTask?.cancel()
        pumpTask = nil
        terminals.values.forEach { $0.stop() }
        terminals.removeAll()
        focusedPaneID = nil
    }

    func focusPane(_ id: Int) {
        guard terminals[id] != nil else {
            return
        }
        focusedPaneID = id
    }

    func focusNextPane() {
        focusPane(offset: 1)
    }

    func focusPreviousPane() {
        focusPane(offset: -1)
    }

    func splitFocused(_ axis: TerminalSplitAxis) {
        let flag = axis == .horizontal ? "-h" : "-v"
        let target = focusedPaneID.map { " -t %\($0)" } ?? ""
        runTmuxCommand("split-window \(flag)\(target)")
    }

    func closeFocusedPane() {
        guard terminals.count > 1, let focusedPaneID else {
            return
        }
        runTmuxCommand("kill-pane -t %\(focusedPaneID)")
    }

    @discardableResult
    func closeFocusedPaneIfSplit() -> Bool {
        guard terminals.count > 1 else {
            return false
        }
        closeFocusedPane()
        return true
    }

    func showSearch() {
        isSearchVisible = true
        isSearchInputFocused = true
        searchFocusRequest &+= 1
    }

    func hideSearch() {
        isSearchVisible = false
        isSearchInputFocused = false
        focusedTerminal?.updateSearch("", options: searchOptions)
    }

    func setSearchInputFocused(_ isFocused: Bool) {
        guard isSearchInputFocused != isFocused else {
            return
        }
        isSearchInputFocused = isFocused
    }

    func clearError() {
        errorMessage = nil
    }

    func toggleSearchCaseSensitive() {
        searchOptions.caseSensitive.toggle()
    }

    func toggleSearchRegex() {
        searchOptions.usesRegex.toggle()
    }

    func send(bytes: [UInt8], toPane paneID: Int? = nil) {
        guard !bytes.isEmpty, let id = paneID ?? focusedPaneID else {
            return
        }
        do {
            try session.sendInput(toPane: id, bytes: bytes)
            terminals[id]?.refreshExternalOutput()
        } catch {
            report(error)
        }
    }

    func send(keyInput: TerminalKeyInput, toPane paneID: Int? = nil) {
        guard let id = paneID ?? focusedPaneID,
              let bytes = terminals[id]?.encodedKeyBytes(keyInput)
        else {
            return
        }
        send(bytes: bytes, toPane: id)
    }

    @discardableResult
    func send(mouseInput: TerminalMouseInput, toPane paneID: Int? = nil) -> Bool {
        guard let id = paneID ?? focusedPaneID,
              let bytes = terminals[id]?.encodedMouseBytes(mouseInput)
        else {
            return false
        }
        send(bytes: bytes, toPane: id)
        return true
    }

    func paste(_ text: String, toPane paneID: Int? = nil) {
        guard let id = paneID ?? focusedPaneID,
              let terminal = terminals[id]
        else {
            return
        }
        send(bytes: terminal.pasteBytes(for: text), toPane: id)
    }

    private func focusPane(offset: Int) {
        let ids = terminals.keys.sorted()
        guard !ids.isEmpty else {
            focusedPaneID = nil
            return
        }
        guard let focusedPaneID,
              let index = ids.firstIndex(of: focusedPaneID)
        else {
            self.focusedPaneID = ids[0]
            return
        }
        self.focusedPaneID = ids[(index + offset + ids.count) % ids.count]
    }

    private func runTmuxCommand(_ command: String) {
        do {
            _ = try session.command(command)
            reconcileLayout()
        } catch {
            report(error)
        }
    }

    @discardableResult
    private func reconcileLayout() -> Bool {
        do {
            guard let nextLayout = try session.reconcileLayout() else {
                throw TmuxControlWorkspaceError.noRenderablePane
            }
            syncTerminals()
            guard !terminals.isEmpty else {
                throw TmuxControlWorkspaceError.noRenderablePane
            }
            if layout != nextLayout {
                layout = nextLayout
            }
            errorMessage = nil
            return true
        } catch {
            report(error)
            return false
        }
    }

    private func pump() {
        guard session.pump() else {
            errorMessage = "tmux control session exited"
            pumpTask?.cancel()
            pumpTask = nil
            return
        }
        syncTerminals()
        if layout != session.layout {
            layout = session.layout
        }
        terminals.values.forEach { $0.refreshExternalOutput() }
    }

    private func syncTerminals() {
        let paneIDs = Set(session.panes.keys)
        for id in Array(terminals.keys) where !paneIDs.contains(id) {
            terminals[id]?.stop()
            terminals.removeValue(forKey: id)
        }
        for id in paneIDs where terminals[id] == nil {
            guard let terminal = session.terminal(forPane: id) else {
                continue
            }
            terminals[id] = TerminalViewModel(displayTerminal: terminal, title: "tmux %\(id)")
        }

        if let focusedPaneID, terminals[focusedPaneID] != nil {
            return
        }
        focusedPaneID = terminals.keys.sorted().first
    }

    private func report(_ error: Error) {
        errorMessage = String(describing: error)
    }

    private static func uniqueTmuxSocketName() -> String {
        "termy-native-\(ProcessInfo.processInfo.processIdentifier)-\(UUID().uuidString.prefix(8))"
    }

    private static func uniqueTmuxSessionName() -> String {
        "termy-native-\(UUID().uuidString.prefix(8))"
    }
}

enum TmuxControlWorkspaceError: Error, CustomStringConvertible {
    case disabled
    case noRenderablePane

    var description: String {
        switch self {
        case .disabled:
            return "tmux control mode is disabled"
        case .noRenderablePane:
            return "tmux control mode did not provide a renderable pane"
        }
    }
}
