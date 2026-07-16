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

    func testKittyGraphicsPlacementsRoundTripThroughFFI() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        let initialRevision = try terminal.kittyGraphicsRevision()
        try terminal.feedOutput(Array(
            "\u{1b}_Ga=T,f=32,s=1,v=1,i=77,c=2,r=1,C=1;AQID/w==\u{1b}\\".utf8
        ))

        let snapshot = try terminal.kittyGraphicsPlacements()

        XCTAssertGreaterThan(snapshot.revision, initialRevision)
        XCTAssertEqual(snapshot.placements.count, 1)
        XCTAssertEqual(snapshot.placements[0].imageID, 77)
        XCTAssertEqual(snapshot.placements[0].displayCols, 2)
        XCTAssertTrue(snapshot.placements[0].png.starts(with: [0x89, 0x50, 0x4E, 0x47]))
    }

    @MainActor
    func testDisplayTerminalViewModelPresentsFedOutput() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        XCTAssertTrue(viewModel.isReady)

        try terminal.feedOutput(Array("tmux pane".utf8))
        viewModel.refreshExternalOutput()

        XCTAssertTrue(viewModel.hasVisibleContent)
        XCTAssertTrue(viewModel.visibleTextSnapshot().contains("tmux pane"))
        viewModel.stop()
        XCTAssertFalse(viewModel.isReady)
    }

    @MainActor
    func testDisplayTerminalPresentsSelectsAndClearsKittyGraphics() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }

        try terminal.feedOutput(Array(
            "\u{1b}_Ga=T,f=32,s=1,v=1,i=91,c=2,r=1,C=1;AQID/w==\u{1b}\\".utf8
        ))
        viewModel.refreshExternalOutput()

        let placement = try XCTUnwrap(viewModel.kittyGraphicsPlacements.first)
        XCTAssertTrue(viewModel.hasVisibleContent)
        let bounds = placement.bounds(renderConfig: viewModel.renderConfig)
        XCTAssertTrue(viewModel.selectKittyGraphics(at: CGPoint(x: bounds.midX, y: bounds.midY)))
        XCTAssertEqual(viewModel.selectedKittyGraphics, placement.identity)
        XCTAssertTrue(viewModel.canCopySelection)

        try terminal.feedOutput(Array("\u{1b}[2J".utf8))
        viewModel.refreshExternalOutput()

        XCTAssertTrue(viewModel.kittyGraphicsPlacements.isEmpty)
        XCTAssertNil(viewModel.selectedKittyGraphics)
        XCTAssertFalse(viewModel.hasVisibleContent)
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
