import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class NativeTabEntranceTests: XCTestCase {
    private func makeBareWindow() -> NSWindow {
        NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
            styleMask: [.titled],
            backing: .buffered,
            defer: true
        )
    }

    /// A freshly opened tab is marked for the entrance animation; other tabs
    /// are not.
    func testMarksFreshlyOpenedTabForEntrance() {
        let manager = NativeTabWindowManager.shared
        let opened = makeBareWindow()
        let other = makeBareWindow()

        manager.noteTabEntrance(for: opened, anchorWindow: nil)

        XCTAssertTrue(manager.shouldAnimateTabEntrance(for: ObjectIdentifier(opened)))
        XCTAssertFalse(manager.shouldAnimateTabEntrance(for: ObjectIdentifier(other)))
    }

    /// The marker is replaced by the next opened tab, so a stale tab never
    /// replays its entrance.
    func testNewerEntranceReplacesOlderMarker() {
        let manager = NativeTabWindowManager.shared
        let first = makeBareWindow()
        let second = makeBareWindow()

        manager.noteTabEntrance(for: first, anchorWindow: nil)
        manager.noteTabEntrance(for: second, anchorWindow: nil)

        XCTAssertFalse(manager.shouldAnimateTabEntrance(for: ObjectIdentifier(first)))
        XCTAssertTrue(manager.shouldAnimateTabEntrance(for: ObjectIdentifier(second)))
    }

    /// Bar entrance only applies to the window that was just opened.
    func testBarEntranceIsScopedToTheOpenedWindow() {
        let manager = NativeTabWindowManager.shared
        let opened = makeBareWindow()
        let other = makeBareWindow()

        manager.noteTabEntrance(for: opened, anchorWindow: nil)

        XCTAssertFalse(manager.shouldAnimateBarEntrance(for: other))
        XCTAssertFalse(manager.shouldAnimateBarEntrance(for: nil))
    }
}
