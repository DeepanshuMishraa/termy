import XCTest
@testable import TermySwift

final class TerminalSurfaceContextMenuTests: XCTestCase {
    @MainActor
    func testTerminalContextMenuIncludesStandardActions() {
        let target = KeyboardCaptureView()

        let menu = TerminalSurfaceContextMenu.make(
            canCopy: true,
            canPaste: true,
            target: target
        )

        XCTAssertEqual(
            menu.items.map(\.title),
            [
                "Copy",
                "Paste",
                "Select All",
                "",
                "Split Right",
                "Split Down",
                "",
                "Clear Scrollback",
                "Search",
            ]
        )
    }

    @MainActor
    func testTerminalContextMenuIncludesCopyImageForGraphicsHit() {
        let target = KeyboardCaptureView()

        let menu = TerminalSurfaceContextMenu.make(
            canCopy: false,
            canCopyImage: true,
            canPaste: false,
            target: target
        )

        XCTAssertEqual(menu.items.prefix(3).map(\.title), ["Copy", "Copy Image", "Paste"])
    }
}
