import XCTest
@testable import TermySwift

final class TmuxControlSessionTests: XCTestCase {
    private enum ExpectedFailure: Error {
        case displayTerminalCreation
    }

    func testPaneIndexParsing() {
        XCTAssertEqual(TmuxControlSession.paneIndex("%3"), 3)
        XCTAssertEqual(TmuxControlSession.paneIndex("7"), 7)
        XCTAssertNil(TmuxControlSession.paneIndex("%x"))
    }

    func testPaneGeometryFromLayout() throws {
        let node = try XCTUnwrap(TmuxLayout.parse("a1b2,80x24,0,0{40x24,0,0,1,39x24,41,0,2}"))
        let geometry = TmuxControlSession.paneGeometry(in: node)
        XCTAssertEqual(geometry.count, 2)
        XCTAssertEqual(geometry[1]?.cols, 40)
        XCTAssertEqual(geometry[1]?.rows, 24)
        XCTAssertEqual(geometry[2]?.cols, 39)
    }

    func testPaneReconciliationPropagatesDisplayTerminalCreationFailure() throws {
        let node = try XCTUnwrap(TmuxLayout.parse("a1b2,80x24,0,0,1"))

        XCTAssertThrowsError(try TmuxControlSession.reconciledPanes(
            current: [:],
            to: node,
            makeDisplayTerminal: { _, _ in
                throw ExpectedFailure.displayTerminalCreation
            }
        )) { error in
            XCTAssertTrue(error is ExpectedFailure)
        }
    }

    func testFailedDisplayTerminalCreationDoesNotPublishLayout() throws {
        let socket = "termy-failed-pane-\(ProcessInfo.processInfo.processIdentifier)-\(UUID().uuidString.prefix(8))"
        let session: TmuxControlSession
        do {
            session = try TmuxControlSession(
                socket: socket,
                session: "failed-pane",
                displayTerminalFactory: { _, _ in
                    throw ExpectedFailure.displayTerminalCreation
                }
            )
        } catch {
            throw XCTSkip("tmux unavailable: \(error)")
        }
        defer { killTmuxServer(socket: socket) }

        XCTAssertThrowsError(try session.reconcileLayout())
        XCTAssertNil(session.layout)
        XCTAssertTrue(session.panes.isEmpty)
    }

    /// End-to-end against real tmux: a split is reconciled into two display
    /// terminals and shell %output is routed into a pane's grid.
    func testReconcileSplitAndRouteOutput() throws {
        let socket = "termy-orch-\(ProcessInfo.processInfo.processIdentifier)"
        let session: TmuxControlSession
        do {
            session = try TmuxControlSession(socket: socket, session: "orch")
        } catch {
            throw XCTSkip("tmux unavailable: \(error)")
        }
        defer { killTmuxServer(socket: socket) }

        try session.reconcileLayout()
        XCTAssertEqual(session.panes.count, 1, "expected one initial pane")

        _ = try session.command("split-window -h")
        try session.reconcileLayout()
        XCTAssertEqual(session.panes.count, 2, "expected two panes after split")

        var routed = false
        for _ in 0..<60 where !routed {
            _ = session.pump()
            for id in session.panes.keys {
                guard let frame = try? session.terminal(forPane: id)?.snapshot() else {
                    continue
                }
                if frame.cells.map(\.character).contains(where: { !$0.isWhitespace }) {
                    routed = true
                    break
                }
            }
            if !routed {
                Thread.sleep(forTimeInterval: 0.05)
            }
        }
        XCTAssertTrue(routed, "expected shell %output routed into a pane grid")
    }

    private func killTmuxServer(socket: String) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["tmux", "-L", socket, "kill-server"]
        try? process.run()
        process.waitUntilExit()
    }
}
