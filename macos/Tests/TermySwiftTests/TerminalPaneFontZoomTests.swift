import XCTest
@testable import TermySwift

@MainActor
final class TerminalPaneFontZoomTests: XCTestCase {
    func testFontZoomChangesOnlyFocusedPane() {
        let store = TerminalWorkspaceStore()
        let originalPane = store.focusedPane
        store.splitFocused(.horizontal)

        guard let originalPane, let focusedPane = store.focusedPane else {
            XCTFail("Expected split workspace to keep two panes")
            return
        }

        let originalFontSize = originalPane.terminal.renderConfig.fontSize
        let focusedFontSize = focusedPane.terminal.renderConfig.fontSize

        focusedPane.terminal.increaseFontSize()

        XCTAssertEqual(originalPane.terminal.renderConfig.fontSize, originalFontSize)
        XCTAssertEqual(focusedPane.terminal.renderConfig.fontSize, focusedFontSize + 1)
    }

    func testFontZoomResetRestoresFocusedPaneDefault() {
        let terminal = TerminalViewModel()
        let originalFontSize = terminal.renderConfig.fontSize
        let originalCellWidth = terminal.renderConfig.cellWidth
        let originalCellHeight = terminal.renderConfig.cellHeight

        terminal.increaseFontSize()
        terminal.increaseFontSize()
        terminal.resetFontSize()

        XCTAssertEqual(terminal.renderConfig.fontSize, originalFontSize)
        XCTAssertEqual(terminal.renderConfig.cellWidth, originalCellWidth)
        XCTAssertEqual(terminal.renderConfig.cellHeight, originalCellHeight)
    }
}
