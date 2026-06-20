import XCTest
@testable import TermySwift

private final class NativeTabNotificationCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0

    var count: Int {
        lock.lock()
        defer {
            lock.unlock()
        }
        return value
    }

    func increment() {
        lock.lock()
        value += 1
        lock.unlock()
    }
}

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

    func testTerminalTitleChangesNotifyNativeTabChrome() {
        let terminal = TerminalViewModel()
        let notificationCounter = NativeTabNotificationCounter()
        let observer = NotificationCenter.default.addObserver(
            forName: .termyNativeTabsChanged,
            object: nil,
            queue: nil
        ) { _ in
            notificationCounter.increment()
        }
        defer {
            NotificationCenter.default.removeObserver(observer)
        }

        XCTAssertTrue(terminal.applyTerminalTitle("  dev server  "))
        XCTAssertEqual(terminal.title, "dev server")
        XCTAssertEqual(notificationCounter.count, 1)

        XCTAssertFalse(terminal.applyTerminalTitle("dev server"))
        XCTAssertFalse(terminal.applyTerminalTitle("   "))
        XCTAssertEqual(notificationCounter.count, 1)

        XCTAssertTrue(terminal.resetTerminalTitle())
        XCTAssertEqual(terminal.title, "Shell")
        XCTAssertEqual(notificationCounter.count, 2)
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
