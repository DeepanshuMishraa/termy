import XCTest
@testable import TermySwift

@MainActor
final class TerminalWorkspaceTabMetadataTests: XCTestCase {
    func testPinnedAndRenamedTabRoundTripsThroughSnapshotRestore() {
        let store = TerminalWorkspaceStore()
        store.setTabPinned(true)
        store.renameTab("dev")

        let snapshot = store.snapshot()

        XCTAssertEqual(snapshot.tabs.first?.pinned, true)
        XCTAssertEqual(snapshot.tabs.first?.manualTitle, "dev")

        let restoredStore = TerminalWorkspaceStore()
        XCTAssertTrue(restoredStore.restore(from: snapshot))
        XCTAssertTrue(restoredStore.tabPinned)
        XCTAssertEqual(restoredStore.tabManualTitle, "dev")
        XCTAssertEqual(restoredStore.tabDisplayTitle, "dev")
    }

    func testBlankRenameClearsManualTabTitle() {
        let store = TerminalWorkspaceStore()
        store.renameTab("  build  ")
        XCTAssertEqual(store.tabManualTitle, "build")

        store.renameTab("   ")

        XCTAssertNil(store.tabManualTitle)
        XCTAssertNil(store.snapshot().tabs.first?.manualTitle)
    }

    func testOpeningSearchAlwaysRequestsInputFocus() {
        let store = TerminalWorkspaceStore()

        store.showSearch()
        let firstRequest = store.searchFocusRequest

        XCTAssertTrue(store.isSearchVisible)
        XCTAssertTrue(store.isSearchInputFocused)
        XCTAssertGreaterThan(firstRequest, 0)

        store.setSearchInputFocused(false)

        XCTAssertTrue(store.isSearchVisible)
        XCTAssertFalse(store.isSearchInputFocused)

        store.showSearch()

        XCTAssertTrue(store.isSearchVisible)
        XCTAssertTrue(store.isSearchInputFocused)
        XCTAssertGreaterThan(store.searchFocusRequest, firstRequest)
    }

    func testClosingSearchClearsInputFocus() {
        let store = TerminalWorkspaceStore()
        store.showSearch()

        store.hideSearch()

        XCTAssertFalse(store.isSearchVisible)
        XCTAssertFalse(store.isSearchInputFocused)
    }
}
