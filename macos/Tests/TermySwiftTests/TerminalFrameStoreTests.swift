import XCTest
@testable import TermySwift

final class TerminalFrameStoreTests: XCTestCase {
    func testFullUpdateReplacesFrame() {
        let store = TerminalFrameStore()
        let frame = TerminalFrame.plainTextPreview("abc", cols: 3, rows: 1)

        let result = store.apply(TerminalFrameUpdate(
            cols: frame.cols,
            rows: frame.rows,
            cells: frame.cells,
            cursor: nil,
            displayOffset: 0,
            historySize: 0,
            damage: .full
        ))

        XCTAssertTrue(result.changed)
        XCTAssertEqual(result.effectiveDamage, .full)
        XCTAssertEqual(store.frame.visibleTextSnapshot(), "abc")
    }

    func testPartialUpdatePatchesOnlyProvidedCells() {
        let store = TerminalFrameStore()
        let initial = TerminalFrame.plainTextPreview("abc", cols: 3, rows: 1)
        store.reset(to: initial)

        var patched = initial.cells[1]
        patched.character = "Z"
        let result = store.apply(TerminalFrameUpdate(
            cols: initial.cols,
            rows: initial.rows,
            cells: [patched],
            cursor: nil,
            displayOffset: 0,
            historySize: 0,
            damage: .partial([TerminalDirtySpan(row: 0, leftCol: 1, rightCol: 1)])
        ))

        XCTAssertTrue(result.changed)
        XCTAssertEqual(result.patchedCellCount, 1)
        XCTAssertEqual(store.frame.cell(row: 0, col: 1).map { String($0.character) }, "Z")
        XCTAssertEqual(result.effectiveDamage.dirtyRows, [0])
    }

    func testCursorOnlyUpdateMarksOldAndNewRowsDirty() {
        let store = TerminalFrameStore()
        var initial = TerminalFrame.plainTextPreview("a\nb", cols: 1, rows: 2)
        initial.cursor = TerminalCursor(col: 0, row: 0, style: .block)
        store.reset(to: initial)

        let result = store.apply(TerminalFrameUpdate(
            cols: initial.cols,
            rows: initial.rows,
            cells: [],
            cursor: TerminalCursor(col: 0, row: 1, style: .block),
            displayOffset: 0,
            historySize: 0,
            damage: .none
        ))

        XCTAssertTrue(result.changed)
        XCTAssertEqual(result.effectiveDamage.dirtyRows, [0, 1])
        XCTAssertEqual(store.frame.cursor?.row, 1)
    }

    func testDisplayOffsetChangeForcesFullDamage() {
        let store = TerminalFrameStore()
        let initial = TerminalFrame.plainTextPreview("abc", cols: 3, rows: 1)
        store.reset(to: initial)

        let result = store.apply(TerminalFrameUpdate(
            cols: initial.cols,
            rows: initial.rows,
            cells: [],
            cursor: nil,
            displayOffset: 1,
            historySize: 4,
            damage: .none
        ))

        XCTAssertTrue(result.changed)
        XCTAssertEqual(result.effectiveDamage, .full)
    }
}
