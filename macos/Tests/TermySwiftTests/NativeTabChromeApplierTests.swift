import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class NativeTabChromeApplierTests: XCTestCase {
    private func makeBareWindow() -> NSWindow {
        NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
            styleMask: [.titled],
            backing: .buffered,
            defer: true
        )
    }

    func testTerminalChromeMirrorsAppearanceAcrossNativeTabGroupWithoutRenamingInactiveTabs() {
        let manager = NativeTabWindowManager.shared
        let first = makeBareWindow()
        let second = makeBareWindow()
        manager.configure(first)
        manager.configure(second)
        first.title = "First"
        second.title = "Second"
        first.addTabbedWindow(second, ordered: .above)

        let background = TerminalRGBA(redByte: 14, greenByte: 22, blueByte: 31, alphaByte: 255)
        let state = TerminalWindowChromeState(
            title: "Active Second",
            isFocused: true,
            background: background,
            backgroundOpacity: 0.8,
            backgroundBlur: false
        )

        XCTAssertTrue(manager.applyTerminalChrome(state, for: second))
        assertColor(first.backgroundColor, matches: background, alpha: 1.0)
        assertColor(second.backgroundColor, matches: background, alpha: 1.0)
        XCTAssertEqual(first.title, "First")
        XCTAssertEqual(second.title, "Active Second")
    }

    private func assertColor(
        _ color: NSColor,
        matches expected: TerminalRGBA,
        alpha: CGFloat,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let color = color.usingColorSpace(.sRGB) ?? color
        XCTAssertEqual(color.redComponent, CGFloat(expected.red), accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(color.greenComponent, CGFloat(expected.green), accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(color.blueComponent, CGFloat(expected.blue), accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(color.alphaComponent, alpha, accuracy: 0.001, file: file, line: line)
    }
}
