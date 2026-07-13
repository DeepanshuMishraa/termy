import AppKit
import Darwin
import XCTest
@testable import TermySwift

@MainActor
final class TmuxControlWorkspaceInteractionTests: XCTestCase {
    private enum ExpectedFailure: Error {
        case displayTerminalCreation
    }

    func testControlWorkspaceDeclinesLaunchWhenTmuxIsDisabled() {
        var configuration = TermyConfigurationStore.shared.configuration
        configuration.tmux.enabled = false

        XCTAssertThrowsError(try TmuxControlWorkspaceModel(configuration: configuration)) { error in
            XCTAssertEqual(String(describing: error), "tmux control mode is disabled")
        }
    }

    func testControlWorkspaceDeclinesLaunchWhenConfiguredBinaryIsMissing() {
        var configuration = TermyConfigurationStore.shared.configuration
        configuration.tmux.enabled = true
        configuration.tmux.binary = "/definitely/missing/termy-tmux"

        XCTAssertThrowsError(try TmuxControlWorkspaceModel(configuration: configuration)) { error in
            XCTAssertEqual(String(describing: error), "tmux control mode is disabled")
        }
    }

    func testControlWorkspaceDeclinesStartWhenDisplayPaneCreationFails() throws {
        let socket = "termy-gui-failed-pane-\(ProcessInfo.processInfo.processIdentifier)-\(UUID().uuidString.prefix(8))"
        let session: TmuxControlSession
        do {
            session = try TmuxControlSession(
                binary: "/opt/homebrew/bin/tmux",
                socket: socket,
                session: "gui-failed-pane",
                displayTerminalFactory: { _, _ in
                    throw ExpectedFailure.displayTerminalCreation
                }
            )
        } catch {
            throw XCTSkip("tmux unavailable: \(error)")
        }
        defer { killTmuxServer(socket: socket) }

        let model = TmuxControlWorkspaceModel(session: session)
        XCTAssertFalse(model.start())
        XCTAssertEqual(model.errorMessage, String(describing: ExpectedFailure.displayTerminalCreation))
        XCTAssertNil(model.layout)
        XCTAssertEqual(model.paneCount, 0)
    }

    func testAppKitInputRoutesToFocusedPaneAcrossNestedLayoutChanges() async throws {
        let socket = "termy-gui-\(ProcessInfo.processInfo.processIdentifier)-\(UUID().uuidString.prefix(8))"
        let session: TmuxControlSession
        do {
            session = try TmuxControlSession(
                binary: "/opt/homebrew/bin/tmux",
                socket: socket,
                session: "gui-smoke",
                loadUserConfig: false
            )
        } catch {
            throw XCTSkip("tmux unavailable: \(error)")
        }
        defer { killTmuxServer(socket: socket) }

        let model = TmuxControlWorkspaceModel(session: session)
        defer { model.stop() }
        model.start()

        let started = await waitUntil {
            model.paneCount == 1 && model.focusedPaneID != nil
        }
        XCTAssertTrue(started, "expected initial tmux pane")
        let firstPaneID = try XCTUnwrap(model.focusedPaneID)
        model.focusedTerminal?.start()

        let harness = AppKitEventHarness()
        harness.inputView.onKeyInput = { model.send(keyInput: $0) }
        harness.inputView.onMouseInput = { model.send(mouseInput: $0) }
        harness.inputView.onPaste = { model.paste($0) }

        let firstMarker = "termy_gui_pane_one"
        typeShellCommand("echo \(firstMarker)", through: harness)
        let firstRouted = await waitUntil {
            model.terminal(forPane: firstPaneID)?.visibleTextSnapshot().contains(firstMarker) == true
        }
        XCTAssertTrue(firstRouted, "expected AppKit keyboard input in the first pane")

        model.splitFocused(.horizontal)
        XCTAssertEqual(model.paneCount, 2)
        model.focusNextPane()
        let secondPaneID = try XCTUnwrap(model.focusedPaneID)
        XCTAssertNotEqual(secondPaneID, firstPaneID)
        model.focusedTerminal?.start()

        let secondMarker = "termy_gui_pane_two"
        typeShellCommand("echo \(secondMarker)", through: harness)
        let secondRouted = await waitUntil {
            model.terminal(forPane: secondPaneID)?.visibleTextSnapshot().contains(secondMarker) == true
        }
        XCTAssertTrue(secondRouted, "expected input in the newly focused pane")
        XCTAssertFalse(
            model.terminal(forPane: firstPaneID)?.visibleTextSnapshot().contains(secondMarker) == true,
            "focused-pane input was routed to the wrong tmux pane"
        )

        model.send(bytes: Array("printf '\\033[?1002h\\033[?1006h'\r".utf8))
        let mouseProbe = TerminalMouseInput(
            kind: .press,
            button: .left,
            position: TerminalGridPosition(col: 1, row: 1),
            control: false,
            alt: false,
            shift: false
        )
        let mouseModeReady = await waitUntil {
            model.focusedTerminal?.encodedMouseBytes(mouseProbe) != nil
        }
        XCTAssertTrue(mouseModeReady, "expected tmux pane to enable SGR mouse reporting")
        if let terminal = model.focusedTerminal {
            harness.inputView.cols = terminal.frame.cols
            harness.inputView.rows = terminal.frame.rows
            harness.inputView.renderConfig = terminal.renderConfig
        }

        typeShellCommand("cat -v", through: harness)
        try? await Task.sleep(for: .milliseconds(100))
        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 1, row: 1))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 1, row: 1))
        let mouseRouted = await waitUntil {
            model.focusedTerminal?.visibleTextSnapshot().contains("^[[<0;") == true
        }
        XCTAssertTrue(mouseRouted, "expected encoded mouse input to reach the focused tmux pane")
        harness.sendKeyDown(keyCode: 0, characters: "c", modifiers: [.control])

        let markerPosition = try XCTUnwrap(position(of: secondMarker, in: model.focusedTerminal?.frame))
        model.focusedTerminal?.updateSelection(TerminalSelection(
            anchor: markerPosition,
            active: TerminalGridPosition(
                col: markerPosition.col + secondMarker.count - 1,
                row: markerPosition.row
            )
        ))
        XCTAssertEqual(model.focusedTerminal?.frame.selectedText(for: model.focusedTerminal?.selection), secondMarker)
        XCTAssertTrue(model.focusedTerminal?.copySelection() == true)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), secondMarker)

        model.showSearch()
        model.focusedTerminal?.updateSearch(secondMarker)
        XCTAssertFalse(model.focusedTerminal?.searchMatches.isEmpty ?? true)
        model.hideSearch()

        model.splitFocused(.vertical)
        XCTAssertEqual(model.paneCount, 3)
        XCTAssertGreaterThanOrEqual(layoutDepth(model.layout), 3)

        _ = try session.command("resize-window -x 120 -y 42")
        let resized = await waitUntil {
            guard let layout = model.layout else {
                return false
            }
            let geometry = TmuxControlSession.paneGeometry(in: layout)
            return !geometry.isEmpty && geometry.allSatisfy { paneID, size in
                guard let frame = model.terminal(forPane: paneID)?.frame else {
                    return false
                }
                return frame.cols == Int(size.cols) && frame.rows == Int(size.rows)
            }
        }
        XCTAssertTrue(resized, "expected live tmux layout sizes to reach every display terminal")

        XCTAssertTrue(model.closeFocusedPaneIfSplit())
        XCTAssertEqual(model.paneCount, 2)
        XCTAssertNotNil(model.focusedPaneID)

        let highOutputPaneID = try XCTUnwrap(model.focusedPaneID)
        typeShellCommand("seq 1 500 | sed 's/^/termy-load-/'", through: harness)
        let highOutputRouted = await waitUntil {
            guard let terminal = model.terminal(forPane: highOutputPaneID) else {
                return false
            }
            return terminal.frame.historySize > 0
                && terminal.visibleTextSnapshot().contains("termy-load-500")
        }
        XCTAssertTrue(highOutputRouted, "expected sustained pane output to reach visible grid and scrollback")
        model.focusedTerminal?.updateSearch("termy-load-250")
        XCTAssertFalse(model.focusedTerminal?.searchMatches.isEmpty ?? true)

        typeShellCommand("exit", through: harness)
        let paneExited = await waitUntil { model.paneCount == 1 }
        XCTAssertTrue(paneExited, "expected an exited pane process to be removed from the workspace")
        XCTAssertNotNil(model.focusedPaneID)

        _ = try session.command("kill-session")
        let sessionExited = await waitUntil {
            model.errorMessage == "tmux control session exited"
        }
        XCTAssertTrue(sessionExited, "expected control-session shutdown to surface in the workspace")
    }

    private func typeShellCommand(_ command: String, through harness: AppKitEventHarness) {
        for character in command {
            harness.sendKeyDown(keyCode: 0, characters: String(character))
        }
        harness.sendKeyDown(keyCode: 36, characters: "\r")
    }

    private func waitUntil(
        attempts: Int = 100,
        condition: @escaping @MainActor () -> Bool
    ) async -> Bool {
        for _ in 0..<attempts {
            if condition() {
                return true
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return condition()
    }

    private func layoutDepth(_ node: TmuxLayoutNode?) -> Int {
        guard let node else {
            return 0
        }
        switch node {
        case .pane:
            return 1
        case let .horizontal(children), let .vertical(children):
            return 1 + (children.map(layoutDepth).max() ?? 0)
        }
    }

    private func position(of text: String, in frame: TerminalFrame?) -> TerminalGridPosition? {
        guard let frame else {
            return nil
        }
        for row in 0..<frame.rows {
            let line = String(frame.cells(inRow: row).map { $0.renderText ? $0.character : " " })
            guard let range = line.range(of: text) else {
                continue
            }
            return TerminalGridPosition(
                col: line.distance(from: line.startIndex, to: range.lowerBound),
                row: row
            )
        }
        return nil
    }

    private func killTmuxServer(socket: String) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/opt/homebrew/bin/tmux")
        process.arguments = ["-L", socket, "kill-server"]
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try? process.run()
        process.waitUntilExit()

        let socketRoot = ProcessInfo.processInfo.environment["TMUX_TMPDIR"] ?? "/tmp"
        let socketPath = "\(socketRoot)/tmux-\(getuid())/\(socket)"
        try? FileManager.default.removeItem(atPath: socketPath)
    }
}
