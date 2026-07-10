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

    @MainActor
    func testDisplayTerminalViewModelPresentsFedOutput() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()

        try terminal.feedOutput(Array("tmux pane".utf8))
        viewModel.refreshExternalOutput()

        XCTAssertTrue(viewModel.hasVisibleContent)
        XCTAssertTrue(viewModel.visibleTextSnapshot().contains("tmux pane"))
        viewModel.stop()
    }

    /// FFI cells carry no position; the binding derives it from row-major
    /// order (full frames) or dirty-span order (partial updates). Pin exact
    /// coordinates through the real library for both paths.
    func testFrameUpdateDerivesCellPositions() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        try terminal.feedOutput(Array("hello".utf8))

        let full = try terminal.frameUpdate(forceFull: true)
        XCTAssertEqual(full.damage, .full)
        let hello = full.cells.filter { "hello".contains($0.character) && $0.renderText }
        XCTAssertEqual(hello.map(\.row), [0, 0, 0, 0, 0])
        XCTAssertEqual(hello.map(\.col), [0, 1, 2, 3, 4])

        // Write "XY" at row 2, col 5 (1-based CUP 3;6) and expect the next
        // update to place exactly those cells there.
        try terminal.feedOutput(Array("\u{1b}[3;6HXY".utf8))
        let update = try terminal.frameUpdate(forceFull: false)
        let x = update.cells.first { $0.character == "X" }
        let y = update.cells.first { $0.character == "Y" }
        XCTAssertEqual(x?.row, 2)
        XCTAssertEqual(x?.col, 5)
        XCTAssertEqual(y?.row, 2)
        XCTAssertEqual(y?.col, 6)
    }
}
