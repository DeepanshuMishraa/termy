import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class TerminalGridOcclusionTests: XCTestCase {
    func testOcclusionDropsLayerBackingStoreAndRepaintsOnReturn() {
        let view = TerminalGridNSView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 200, height: 100),
            styleMask: [.titled],
            backing: .buffered,
            defer: true
        )
        window.contentView = view
        XCTAssertNotNil(view.layer)

        // Stand in for the backing store the window server gives a drawn layer.
        view.layer?.contents = NSImage(size: NSSize(width: 1, height: 1))

        view.applyWindowOcclusion(visible: false)
        XCTAssertNil(view.layer?.contents, "occluded view must release its layer backing store")

        view.needsDisplay = false
        view.applyWindowOcclusion(visible: true)
        XCTAssertTrue(view.needsDisplay, "returning to visibility must schedule a repaint")
    }
}
