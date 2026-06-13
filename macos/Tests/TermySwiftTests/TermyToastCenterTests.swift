import XCTest
@testable import TermySwift

@MainActor
final class TermyToastCenterTests: XCTestCase {
    func testShowAppendsAndCapsVisible() {
        let center = TermyToastCenter()
        center.show("one", autoDismiss: false)
        center.show("two", autoDismiss: false)
        center.show("three", autoDismiss: false)
        center.show("four", autoDismiss: false)

        XCTAssertEqual(center.toasts.count, TermyToastCenter.maxVisible)
        // Oldest dropped once over the cap.
        XCTAssertEqual(center.toasts.map(\.message), ["two", "three", "four"])
    }

    func testEmptyMessageIgnored() {
        let center = TermyToastCenter()
        center.show("   ", autoDismiss: false)
        XCTAssertTrue(center.toasts.isEmpty)
    }

    func testDismissRemovesByID() {
        let center = TermyToastCenter()
        center.show("keep", autoDismiss: false)
        center.show("drop", autoDismiss: false)
        let dropID = center.toasts[1].id

        center.dismiss(dropID)
        XCTAssertEqual(center.toasts.map(\.message), ["keep"])
    }
}
