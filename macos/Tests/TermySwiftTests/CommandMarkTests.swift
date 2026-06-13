import XCTest
@testable import TermySwift

final class CommandMarkTests: XCTestCase {
    func testAbsoluteRowCombinesHistoryAndCursor() {
        XCTAssertEqual(TerminalViewModel.commandMarkAbsoluteRow(historySize: 120, cursorRow: 4), 124)
        XCTAssertEqual(TerminalViewModel.commandMarkAbsoluteRow(historySize: 0, cursorRow: 0), 0)
    }

    func testTracksOnlyBelowScrollbackCap() {
        // Below the cap: stable, so tracking is allowed.
        XCTAssertTrue(TerminalViewModel.canTrackCommandMark(historySize: 500, scrollbackCap: 1000))
        // At/over the cap: eviction would drift marks, so tracking stops.
        XCTAssertFalse(TerminalViewModel.canTrackCommandMark(historySize: 1000, scrollbackCap: 1000))
        XCTAssertFalse(TerminalViewModel.canTrackCommandMark(historySize: 1200, scrollbackCap: 1000))
        // Unknown cap (0) disables tracking rather than risk drift.
        XCTAssertFalse(TerminalViewModel.canTrackCommandMark(historySize: 0, scrollbackCap: 0))
    }

    func testPlainWakeupsDoNotCountAsCadenceActivity() {
        XCTAssertFalse(TerminalViewModel.hasActivityEvents([]))
        XCTAssertFalse(TerminalViewModel.hasActivityEvents([.wakeup, .wakeup]))
        XCTAssertTrue(TerminalViewModel.hasActivityEvents([.wakeup, .title("ready")]))
    }

    func testCadenceActivityRequiresMeaningfulEventContentPatchOrForcedRefresh() {
        XCTAssertFalse(TerminalViewModel.hasCadenceActivity(
            events: [.wakeup],
            patchedCellCount: 0,
            forceFull: false,
            changed: true
        ))
        XCTAssertTrue(TerminalViewModel.hasCadenceActivity(
            events: [],
            patchedCellCount: 1,
            forceFull: false,
            changed: true
        ))
        XCTAssertTrue(TerminalViewModel.hasCadenceActivity(
            events: [],
            patchedCellCount: 0,
            forceFull: true,
            changed: true
        ))
    }
}
