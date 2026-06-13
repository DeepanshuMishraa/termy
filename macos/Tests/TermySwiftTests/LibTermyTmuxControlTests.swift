import XCTest
@testable import TermySwift

/// Exercises the Swift → FFI → tmux control-mode chain. Skips cleanly when tmux
/// isn't installed so the suite still runs in environments without it.
final class LibTermyTmuxControlTests: XCTestCase {
    func testControlSessionReceivesNotifications() throws {
        let socket = "termy-swift-tmux-\(ProcessInfo.processInfo.processIdentifier)"
        let control: LibTermyTmuxControl
        do {
            control = try LibTermyTmuxControl(binary: "tmux", socket: socket, session: "termyswift")
        } catch {
            throw XCTSkip("tmux unavailable: \(error)")
        }

        var received = false
        for _ in 0..<40 where !received {
            received = !control.poll().isEmpty
            if !received {
                Thread.sleep(forTimeInterval: 0.05)
            }
        }
        XCTAssertTrue(received, "expected tmux control-mode notifications via FFI")

        let output = try control.send("display-message -p termy-swift-ok")
        XCTAssertFalse(output.isEmpty)

        killTmuxServer(socket: socket)
    }

    private func killTmuxServer(socket: String) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["tmux", "-L", socket, "kill-server"]
        try? process.run()
        process.waitUntilExit()
    }
}
