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
        // A serious/critical floor must be slower than the full-speed active tick.
        XCTAssertGreaterThan(DisplaySyncedRefreshDriver.thermalFloor(.serious), RefreshCadence.active.interval)
        XCTAssertGreaterThan(DisplaySyncedRefreshDriver.thermalFloor(.critical), RefreshCadence.active.interval)
    }

    func testIdleCadenceTracksCursorBlinkInsteadOfFramePolling() {
        XCTAssertEqual(RefreshCadence.idle.interval, TerminalCursorBlinkPhase.interval, accuracy: 1e-9)
        XCTAssertGreaterThan(RefreshCadence.idle.interval, 1.0 / 15.0)
    }

    func testInertIdleCadenceIsSlowerThanBlinkCadence() {
        // With nothing to animate, the idle poll is only a safety net — it must
        // be meaningfully slower than blink-rate polling.
        XCTAssertGreaterThan(RefreshCadence.idleInert.interval, RefreshCadence.idle.interval * 2)
    }

    func testIdleCadenceSelectionFollowsBlinkRelevance() {
        // Only a focused pane with blink enabled animates a cursor; everything
        // else idles at the inert cadence.
        XCTAssertEqual(TerminalViewModel.idleCadence(cursorBlink: true, paneFocused: true), .idle)
        XCTAssertEqual(TerminalViewModel.idleCadence(cursorBlink: false, paneFocused: true), .idleInert)
        XCTAssertEqual(TerminalViewModel.idleCadence(cursorBlink: true, paneFocused: false), .idleInert)
        XCTAssertEqual(TerminalViewModel.idleCadence(cursorBlink: false, paneFocused: false), .idleInert)
    }

    func testDispatchGapStaysBelowDeliveryInterval() {
        // The CoreVideo-thread pre-filter must never starve the main-thread
        // delivery check: its gap has to sit strictly below the interval it
        // protects, or link-fire jitter could drop due ticks.
        let interval = RefreshCadence.active.interval
        let gap = DisplaySyncedRefreshDriver.dispatchGap(deliveryInterval: interval)
        XCTAssertGreaterThan(gap, 0)
        XCTAssertLessThan(gap, interval)
    }
}
