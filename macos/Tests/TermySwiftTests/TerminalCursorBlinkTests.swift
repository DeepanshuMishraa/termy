import XCTest
@testable import TermySwift

final class TerminalCursorBlinkTests: XCTestCase {
    private let start = Date(timeIntervalSinceReferenceDate: 0)

    func testTogglesAfterEachInterval() {
        var phase = TerminalCursorBlinkPhase()

        XCTAssertFalse(phase.tick(blinkEnabled: true, now: start))
        XCTAssertFalse(phase.tick(blinkEnabled: true, now: start.addingTimeInterval(0.2)))
        XCTAssertTrue(phase.isVisible)

        XCTAssertTrue(phase.tick(blinkEnabled: true, now: start.addingTimeInterval(0.6)))
        XCTAssertFalse(phase.isVisible)

        XCTAssertTrue(phase.tick(blinkEnabled: true, now: start.addingTimeInterval(1.2)))
        XCTAssertTrue(phase.isVisible)
    }

    func testDisabledBlinkKeepsCursorVisible() {
        var phase = TerminalCursorBlinkPhase()
        _ = phase.tick(blinkEnabled: true, now: start)
        _ = phase.tick(blinkEnabled: true, now: start.addingTimeInterval(0.6))
        XCTAssertFalse(phase.isVisible)

        XCTAssertTrue(phase.tick(blinkEnabled: false, now: start.addingTimeInterval(0.7)))
        XCTAssertTrue(phase.isVisible)
        XCTAssertFalse(phase.tick(blinkEnabled: false, now: start.addingTimeInterval(2)))
        XCTAssertTrue(phase.isVisible)
    }

    func testInputHoldsCursorSolidForOneInterval() {
        var phase = TerminalCursorBlinkPhase()
        _ = phase.tick(blinkEnabled: true, now: start)
        _ = phase.tick(blinkEnabled: true, now: start.addingTimeInterval(0.6))
        XCTAssertFalse(phase.isVisible)

        phase.noteInput(at: start.addingTimeInterval(0.7))
        XCTAssertTrue(phase.isVisible)

        // Ticks inside the hold window keep the cursor solid and re-phase the
        // toggle, so blinking resumes a full interval after typing stops.
        XCTAssertFalse(phase.tick(blinkEnabled: true, now: start.addingTimeInterval(1.0)))
        XCTAssertTrue(phase.isVisible)
        XCTAssertFalse(phase.tick(blinkEnabled: true, now: start.addingTimeInterval(1.3)))
        XCTAssertTrue(phase.isVisible)
        XCTAssertTrue(phase.tick(blinkEnabled: true, now: start.addingTimeInterval(1.6)))
        XCTAssertFalse(phase.isVisible)
    }

    func testDamageUnionPreservesExistingSpans() {
        let span = TerminalDirtySpan(row: 3, leftCol: 2, rightCol: 2)
        XCTAssertEqual(TerminalDamage.full.union(span), .full)
        XCTAssertEqual(TerminalDamage.none.union(span), .partial([span]))

        let existing = TerminalDirtySpan(row: 1, leftCol: 0, rightCol: 4)
        XCTAssertEqual(
            TerminalDamage.partial([existing]).union(span),
            .partial([existing, span])
        )
    }

    @MainActor
    func testResetCursorBlinkPhaseForcesVisibleCursor() {
        let terminal = TerminalViewModel()
        terminal.resetCursorBlinkPhase()
        XCTAssertTrue(terminal.cursorBlinkVisible)
    }
}
