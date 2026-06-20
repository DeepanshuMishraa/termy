import XCTest
@testable import TermySwift

@MainActor
final class TerminalPaneMoveTests: XCTestCase {
    func testMovingPaneLeftOfTargetPreservesTerminalAndFocus() {
        let store = TerminalWorkspaceStore()
        guard let firstPane = store.focusedPane else {
            XCTFail("Expected initial pane")
            return
        }

        store.splitFocused(.horizontal)
        guard let secondPane = store.focusedPane else {
            XCTFail("Expected split pane")
            return
        }
        let secondTerminal = secondPane.terminal

        XCTAssertTrue(store.movePane(secondPane.id, to: firstPane.id, placement: .left))

        XCTAssertEqual(store.focusedPane?.id, secondPane.id)
        XCTAssertTrue(store.focusedPane?.terminal === secondTerminal)

        let snapshot = store.snapshot()
        XCTAssertEqual(snapshot.tabs.first?.panes.map(\.id), [secondPane.id, firstPane.id])
        guard case let .split(axis, ratio, first, second) = snapshot.tabs.first?.layoutTree else {
            XCTFail("Expected moved panes to remain split")
            return
        }
        XCTAssertEqual(axis, .horizontal)
        XCTAssertEqual(ratio, 0.5)
        XCTAssertEqual(first, .leaf(pane: 0))
        XCTAssertEqual(second, .leaf(pane: 1))
    }

    func testMovingPaneBelowTargetBuildsVerticalSplit() {
        let store = TerminalWorkspaceStore()
        let firstPaneID = store.focusedPaneID
        store.splitFocused(.horizontal)
        let secondPaneID = store.focusedPaneID

        XCTAssertTrue(store.movePane(firstPaneID, to: secondPaneID, placement: .bottom))

        let snapshot = store.snapshot()
        XCTAssertEqual(snapshot.tabs.first?.panes.map(\.id), [secondPaneID, firstPaneID])
        guard case let .split(axis, ratio, first, second) = snapshot.tabs.first?.layoutTree else {
            XCTFail("Expected moved panes to remain split")
            return
        }
        XCTAssertEqual(axis, .vertical)
        XCTAssertEqual(ratio, 0.5)
        XCTAssertEqual(first, .leaf(pane: 0))
        XCTAssertEqual(second, .leaf(pane: 1))
    }

    func testMovingPaneOntoItselfIsNoOp() {
        let store = TerminalWorkspaceStore()
        let paneID = store.focusedPaneID

        XCTAssertFalse(store.movePane(paneID, to: paneID, placement: .right))
        XCTAssertEqual(store.snapshot().tabs.first?.panes.map(\.id), [paneID])
    }
}
