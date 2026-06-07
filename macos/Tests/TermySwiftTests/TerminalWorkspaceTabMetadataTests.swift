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
}
