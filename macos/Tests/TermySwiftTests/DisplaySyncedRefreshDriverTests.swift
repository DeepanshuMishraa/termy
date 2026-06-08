import XCTest
@testable import TermySwift

final class DisplaySyncedRefreshDriverTests: XCTestCase {
    func testThermalFloorThrottlesUnderPressure() {
        XCTAssertEqual(DisplaySyncedRefreshDriver.thermalFloor(.nominal), 0)
        XCTAssertEqual(DisplaySyncedRefreshDriver.thermalFloor(.fair), 0)
        XCTAssertEqual(DisplaySyncedRefreshDriver.thermalFloor(.serious), 1.0 / 30.0, accuracy: 1e-9)
        XCTAssertEqual(DisplaySyncedRefreshDriver.thermalFloor(.critical), 1.0 / 15.0, accuracy: 1e-9)
    }

    func testThermalFloorNeverFasterThanActiveCadence() {
        // A serious/critical floor must not be quicker than the 60 Hz active tick.
        XCTAssertGreaterThan(DisplaySyncedRefreshDriver.thermalFloor(.serious), RefreshCadence.active.interval)
        XCTAssertGreaterThan(DisplaySyncedRefreshDriver.thermalFloor(.critical), RefreshCadence.active.interval)
    }
}
