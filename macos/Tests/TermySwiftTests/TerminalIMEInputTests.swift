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
}
