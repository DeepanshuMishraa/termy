import XCTest
@testable import TermySwift

/// Covers the text extraction that backs the terminal grid's VoiceOver value.
final class TerminalAccessibilityTests: XCTestCase {
    func testVisibleTextSnapshotReadsRows() {
        let frame = TerminalFrame.plainTextPreview("hello\nworld", cols: 10, rows: 4)
        XCTAssertEqual(frame.visibleTextSnapshot(), "hello\nworld")
    }

    func testEmptyFrameHasNoAccessibilityText() {
        XCTAssertEqual(TerminalFrame.empty.visibleTextSnapshot(), "")
    }

    func testSelectedTextBacksAccessibilitySelection() {
        let frame = TerminalFrame.plainTextPreview("hello\nworld", cols: 10, rows: 4)
        // Rows are top-padded to fill `rows`; "hello" sits on row 2, "world" row 3.
        let selection = TerminalSelection(
            anchor: TerminalGridPosition(col: 0, row: 2),
            active: TerminalGridPosition(col: 4, row: 2)
        )
        XCTAssertEqual(frame.selectedText(for: selection), "hello")
        XCTAssertNil(frame.selectedText(for: nil))
    }

    /// `hasSelectedText` is the allocation-free predicate the surface view uses
    /// per render pass; it must agree exactly with `selectedText` non-emptiness.
    func testHasSelectedTextMatchesSelectedTextNonEmptiness() {
        let frame = TerminalFrame.plainTextPreview("hello\nworld", cols: 10, rows: 4)
        let selections: [TerminalSelection?] = [
            nil,
            // Single row over text.
            TerminalSelection(
                anchor: TerminalGridPosition(col: 0, row: 2),
                active: TerminalGridPosition(col: 4, row: 2)
            ),
            // Single row over blank padding only.
            TerminalSelection(
                anchor: TerminalGridPosition(col: 6, row: 2),
                active: TerminalGridPosition(col: 9, row: 2)
            ),
            // Blank row only.
            TerminalSelection(
                anchor: TerminalGridPosition(col: 0, row: 0),
                active: TerminalGridPosition(col: 9, row: 0)
            ),
            // Multi-row across blank rows (joined with a newline, so non-empty).
            TerminalSelection(
                anchor: TerminalGridPosition(col: 0, row: 0),
                active: TerminalGridPosition(col: 9, row: 1)
            ),
            // Multi-row across text, reversed anchor/active.
            TerminalSelection(
                anchor: TerminalGridPosition(col: 4, row: 3),
                active: TerminalGridPosition(col: 0, row: 2)
            )
        ]

        for selection in selections {
            let text = frame.selectedText(for: selection)
            XCTAssertEqual(
                frame.hasSelectedText(for: selection),
                !(text ?? "").isEmpty,
                "mismatch for selection \(String(describing: selection))"
            )
        }

        XCTAssertFalse(TerminalFrame.empty.hasSelectedText(for: TerminalSelection(
            anchor: TerminalGridPosition(col: 0, row: 0),
            active: TerminalGridPosition(col: 4, row: 0)
        )))
    }
}
