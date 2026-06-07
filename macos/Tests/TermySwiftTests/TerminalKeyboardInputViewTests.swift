import XCTest
@testable import TermySwift

final class TerminalKeyboardInputViewTests: XCTestCase {
    func testDragSelectionIgnoresSameCellJitter() {
        let position = TerminalGridPosition(col: 12, row: 4)

        XCTAssertNil(KeyboardCaptureView.dragSelection(anchor: position, active: position))
    }

    func testDragSelectionStartsAfterMovingToAnotherCell() {
        let anchor = TerminalGridPosition(col: 12, row: 4)
        let active = TerminalGridPosition(col: 13, row: 4)

        XCTAssertEqual(
            KeyboardCaptureView.dragSelection(anchor: anchor, active: active),
            TerminalSelection(anchor: anchor, active: active)
        )
    }
}
