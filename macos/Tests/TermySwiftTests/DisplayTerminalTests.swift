import XCTest
@testable import TermySwift

/// Verifies the display-only terminal binding used for tmux control-mode panes:
/// bytes fed via `feedOutput` land in the grid, and `write` is a safe no-op.
final class DisplayTerminalTests: XCTestCase {
    func testFeedOutputAppearsInGrid() throws {
        let terminal = try LibTermyTerminal(displayCols: 80, rows: 24, loadUserConfig: false)
        try terminal.feedOutput(Array("hello world".utf8))
        let frame = try terminal.snapshot()
        let text = String(frame.cells.map(\.character))
        XCTAssertTrue(text.contains("hello world"), "display grid missing fed output")
    }

    func testWriteIsNoOpForDisplayTerminal() throws {
        let terminal = try LibTermyTerminal(displayCols: 80, rows: 24, loadUserConfig: false)
        XCTAssertNoThrow(try terminal.write(Array("ignored".utf8)))
    }
}
