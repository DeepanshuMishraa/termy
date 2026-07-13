import XCTest
@testable import TermySwift

@MainActor
final class TerminalSelectionInteractionTests: XCTestCase {
    func testDragBeyondPaneBoundsClampsSelectionToOriginatingPane() throws {
        let firstTerminal = try LibTermyTerminal(displayCols: 4, rows: 2, loadUserConfig: false)
        let secondTerminal = try LibTermyTerminal(displayCols: 4, rows: 2, loadUserConfig: false)
        try firstTerminal.feedOutput(Array("abcd".utf8))
        try secondTerminal.feedOutput(Array("WXYZ".utf8))
        let firstPane = TerminalViewModel(displayTerminal: firstTerminal)
        let secondPane = TerminalViewModel(displayTerminal: secondTerminal)
        firstPane.start()
        secondPane.start()
        defer {
            firstPane.stop()
            secondPane.stop()
        }
        firstPane.refreshExternalOutput()
        secondPane.refreshExternalOutput()

        let harness = AppKitEventHarness(size: NSSize(width: 200, height: 100))
        harness.inputView.cols = firstPane.frame.cols
        harness.inputView.rows = firstPane.frame.rows
        harness.inputView.renderConfig = firstPane.renderConfig
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectionChanged = { firstPane.updateSelection($0) }

        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 1, row: 0))
        let beyondPane = NSPoint(x: 600, y: harness.point(col: 3, row: 0).y)
        harness.sendMouse(type: .leftMouseDragged, at: beyondPane)
        harness.sendMouse(type: .leftMouseUp, at: beyondPane)

        let selection = try XCTUnwrap(firstPane.selection)
        XCTAssertEqual(selection.active.col, firstPane.frame.cols - 1)
        XCTAssertEqual(firstPane.frame.selectedText(for: selection), "bcd")
        XCTAssertNil(secondPane.selection)
    }

    func testAppKitDragSelectionCopiesRowsFromScrollbackViewport() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        let output = (1...12).map { "line-\($0)\r\n" }.joined()
        try terminal.feedOutput(Array(output.utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()
        viewModel.scrollToTop()

        XCTAssertGreaterThan(viewModel.frame.displayOffset, 0)
        XCTAssertTrue(viewModel.frame.visibleTextSnapshot().hasPrefix("line-1\nline-2"))

        let harness = AppKitEventHarness()
        harness.inputView.cols = viewModel.frame.cols
        harness.inputView.rows = viewModel.frame.rows
        harness.inputView.renderConfig = viewModel.renderConfig
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectionChanged = { viewModel.updateSelection($0) }

        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 0, row: 0))
        harness.sendMouse(type: .leftMouseDragged, at: harness.point(col: 5, row: 1))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 5, row: 1))

        let selection = try XCTUnwrap(viewModel.selection)
        XCTAssertEqual(viewModel.frame.selectedText(for: selection), "line-1\nline-2")
    }

    func testAppKitDragSelectionDoesNotInventNewlineAcrossSoftWrap() throws {
        let terminal = try LibTermyTerminal(displayCols: 4, rows: 2, loadUserConfig: false)
        try terminal.feedOutput(Array("abcde".utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()

        let harness = AppKitEventHarness()
        harness.inputView.cols = viewModel.frame.cols
        harness.inputView.rows = viewModel.frame.rows
        harness.inputView.renderConfig = viewModel.renderConfig
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectionChanged = { viewModel.updateSelection($0) }

        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 0, row: 0))
        harness.sendMouse(type: .leftMouseDragged, at: harness.point(col: 0, row: 1))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 0, row: 1))

        let selection = try XCTUnwrap(viewModel.selection)
        XCTAssertEqual(viewModel.frame.selectedText(for: selection), "abcde")
        XCTAssertTrue(viewModel.canCopySelection)
    }

    func testAppKitDragSelectionPreservesWideGlyphText() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        try terminal.feedOutput(Array("A界B".utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()

        let harness = AppKitEventHarness()
        harness.inputView.cols = viewModel.frame.cols
        harness.inputView.rows = viewModel.frame.rows
        harness.inputView.renderConfig = viewModel.renderConfig
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectionChanged = { viewModel.updateSelection($0) }

        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 0, row: 0))
        harness.sendMouse(type: .leftMouseDragged, at: harness.point(col: 3, row: 0))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 3, row: 0))

        let selection = try XCTUnwrap(viewModel.selection)
        XCTAssertEqual(viewModel.frame.selectedText(for: selection), "A界B")
        XCTAssertTrue(viewModel.canCopySelection)
    }

    func testSelectionRemainsValidWhenOutputChanges() throws {
        let terminal = try LibTermyTerminal(displayCols: 20, rows: 4, loadUserConfig: false)
        try terminal.feedOutput(Array("selected text".utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()

        let harness = AppKitEventHarness()
        harness.inputView.cols = viewModel.frame.cols
        harness.inputView.rows = viewModel.frame.rows
        harness.inputView.renderConfig = viewModel.renderConfig
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectionChanged = { viewModel.updateSelection($0) }
        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 0, row: 0))
        harness.sendMouse(type: .leftMouseDragged, at: harness.point(col: 7, row: 0))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 7, row: 0))
        XCTAssertTrue(viewModel.canCopySelection)

        try terminal.feedOutput(Array("\r\nmore output".utf8))
        viewModel.refreshExternalOutput()

        XCTAssertNotNil(viewModel.selection)
        XCTAssertTrue(viewModel.canCopySelection)
    }
}
