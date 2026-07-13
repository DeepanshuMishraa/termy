import XCTest
@testable import TermySwift

@MainActor
final class TerminalIMEInputTests: XCTestCase {
    func testMarkedTextTrackingThroughComposition() {
        let view = KeyboardCaptureView(frame: .zero)
        XCTAssertFalse(view.hasMarkedText())
        XCTAssertEqual(view.markedRange().location, NSNotFound)

        view.setMarkedText(
            "か",
            selectedRange: NSRange(location: 1, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0)
        )
        XCTAssertTrue(view.hasMarkedText())
        XCTAssertEqual(view.markedRange(), NSRange(location: 0, length: 1))

        view.unmarkText()
        XCTAssertFalse(view.hasMarkedText())
    }

    /// The no-regression guarantee: a stray `insertText` with no active
    /// composition must emit nothing, so plain keys fall through to the terminal
    /// encoder in `keyDown` instead of being double-typed.
    func testPlainInsertWithoutCompositionEmitsNothing() {
        let view = KeyboardCaptureView(frame: .zero)
        var emitted: [[UInt8]] = []
        view.onBytes = { emitted.append($0) }

        view.insertText("a", replacementRange: NSRange(location: NSNotFound, length: 0))

        XCTAssertTrue(emitted.isEmpty)
        XCTAssertFalse(view.hasMarkedText())
    }

    /// Committing while composing clears the marked text.
    func testCommitClearsMarkedText() {
        let view = KeyboardCaptureView(frame: .zero)
        view.setMarkedText(
            "k",
            selectedRange: NSRange(location: 1, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0)
        )
        XCTAssertTrue(view.hasMarkedText())

        view.insertText("か", replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertFalse(view.hasMarkedText())
    }

    func testInternationalCompositionCommitsUTF8Bytes() {
        let view = KeyboardCaptureView(frame: .zero)
        var emitted: [[UInt8]] = []
        view.onBytes = { emitted.append($0) }

        for text in ["かな", "中文", "한글", "é"] {
            view.setMarkedText(
                text,
                selectedRange: NSRange(location: text.utf16.count, length: 0),
                replacementRange: NSRange(location: NSNotFound, length: 0)
            )
            view.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
        }

        XCTAssertEqual(emitted, ["かな", "中文", "한글", "é"].map { Array($0.utf8) })
        XCTAssertFalse(view.hasMarkedText())
    }

    func testCandidateRectTracksCursorCellAndViewResize() {
        let harness = AppKitEventHarness(size: NSSize(width: 500, height: 300))
        let view = harness.inputView
        view.cols = 20
        view.rows = 10
        view.cursorPosition = TerminalGridPosition(col: 7, row: 3)

        let first = view.firstRect(
            forCharacterRange: NSRange(location: 0, length: 0),
            actualRange: nil
        )
        let expectedFirst = harness.window.convertToScreen(view.convert(
            NSRect(
                x: view.renderConfig.paddingX + 7 * view.renderConfig.cellWidth,
                y: view.bounds.height - view.renderConfig.paddingY - 4 * view.renderConfig.cellHeight,
                width: view.renderConfig.cellWidth,
                height: view.renderConfig.cellHeight
            ),
            to: nil
        ))
        XCTAssertEqual(first, expectedFirst)

        harness.window.setContentSize(NSSize(width: 700, height: 480))
        let resized = view.firstRect(
            forCharacterRange: NSRange(location: 0, length: 0),
            actualRange: nil
        )
        let expectedResized = harness.window.convertToScreen(view.convert(
            NSRect(
                x: view.renderConfig.paddingX + 7 * view.renderConfig.cellWidth,
                y: view.bounds.height - view.renderConfig.paddingY - 4 * view.renderConfig.cellHeight,
                width: view.renderConfig.cellWidth,
                height: view.renderConfig.cellHeight
            ),
            to: nil
        ))
        XCTAssertEqual(resized, expectedResized)
        XCTAssertEqual(resized.width, first.width, accuracy: 0.001)
        XCTAssertEqual(resized.height, first.height, accuracy: 0.001)
    }
}
