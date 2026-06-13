import Foundation

/// Orchestrates a tmux control-mode session: reconciles the tmux window layout
/// into a set of display terminals, routes `%output` to each pane, and forwards
/// input via `send-keys`. UI-agnostic and testable; the SwiftUI workspace renders
/// `layout` + `terminal(forPane:)`. Not thread-safe — drive from one actor.
final class TmuxControlSession {
    private let control: LibTermyTmuxControl
    private let loadUserConfig: Bool

    /// The most recently reconciled tmux layout (pane tree).
    private(set) var layout: TmuxLayoutNode?
    /// Display terminals keyed by tmux pane id (the integer in `%N`).
    private(set) var panes: [Int: LibTermyTerminal] = [:]

    init(control: LibTermyTmuxControl, loadUserConfig: Bool = false) {
        self.control = control
        self.loadUserConfig = loadUserConfig
    }

    convenience init(
        binary: String = "tmux",
        socket: String,
        session: String,
        loadUserConfig: Bool = false
    ) throws {
        let control = try LibTermyTmuxControl(binary: binary, socket: socket, session: session)
        self.init(control: control, loadUserConfig: loadUserConfig)
    }

    /// Run an arbitrary tmux command over the control channel (e.g. `split-window`).
    @discardableResult
    func command(_ command: String) throws -> String {
        try control.send(command)
    }

    /// Forward input bytes to a tmux pane via hex-encoded `send-keys`.
    func sendInput(toPane id: Int, bytes: [UInt8]) throws {
        guard !bytes.isEmpty else {
            return
        }
        let hex = bytes.map { String(format: "%02x", $0) }.joined(separator: " ")
        _ = try control.send("send-keys -t %\(id) -H \(hex)")
    }

    func terminal(forPane id: Int) -> LibTermyTerminal? {
        panes[id]
    }

    /// Query the current tmux layout and reconcile the display-terminal set.
    @discardableResult
    func reconcileLayout() throws -> TmuxLayoutNode? {
        let raw = try control.send("display-message -p \"#{window_layout}\"")
        guard let line = raw.split(whereSeparator: \.isNewline).first(where: { !$0.isEmpty }),
              let parsed = TmuxLayout.parse(String(line))
        else {
            return layout
        }
        reconcile(to: parsed)
        layout = parsed
        return layout
    }

    /// Drain pending control notifications: route `%output` to panes, refresh on
    /// layout changes. Returns `false` once the session has exited.
    @discardableResult
    func pump() -> Bool {
        var alive = true
        for notification in control.poll() {
            switch notification {
            case let .output(paneID, bytes):
                if let id = Self.paneIndex(paneID), let terminal = panes[id] {
                    try? terminal.feedOutput(bytes)
                }
            case .needsRefresh:
                _ = try? reconcileLayout()
            case .warning:
                break
            case .exit:
                alive = false
            }
        }
        return alive
    }

    private func reconcile(to node: TmuxLayoutNode) {
        let wanted = Self.paneGeometry(in: node)
        for id in panes.keys where wanted[id] == nil {
            panes.removeValue(forKey: id)
        }
        for (id, size) in wanted {
            if let terminal = panes[id] {
                try? terminal.resize(
                    cols: size.cols,
                    rows: size.rows,
                    cellWidth: Float(terminal.renderConfig.cellWidth),
                    cellHeight: Float(terminal.renderConfig.cellHeight)
                )
            } else if let terminal = try? LibTermyTerminal(
                displayCols: size.cols,
                rows: size.rows,
                loadUserConfig: loadUserConfig
            ) {
                panes[id] = terminal
            }
        }
    }

    /// Map of pane id → cell dimensions for every leaf in a layout tree.
    static func paneGeometry(in node: TmuxLayoutNode) -> [Int: (cols: UInt16, rows: UInt16)] {
        switch node {
        case let .pane(id, width, height, _, _):
            // `clamping:` rather than the trapping `UInt16(_:)`: a buggy/hostile
            // tmux `#{window_layout}` reply can carry out-of-range dimensions,
            // and a parser trap here would crash the app.
            return [id: (UInt16(clamping: max(1, width)), UInt16(clamping: max(1, height)))]
        case let .horizontal(children), let .vertical(children):
            return children.reduce(into: [:]) { accumulated, child in
                accumulated.merge(paneGeometry(in: child)) { existing, _ in existing }
            }
        }
    }

    /// Parse the integer pane index from a tmux pane id such as `"%1"`.
    static func paneIndex(_ paneID: String) -> Int? {
        Int(paneID.drop(while: { $0 == "%" }))
    }
}
