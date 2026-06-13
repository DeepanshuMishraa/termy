import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class TerminalSurfaceViewTests: XCTestCase {
    func testInactivePaneContentOpacityFollowsFocusEffect() {
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: true,
                effect: .softSpotlight,
                strength: 0.6
            ),
            1.0
        )
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: false,
                isFocused: false,
                effect: .softSpotlight,
                strength: 0.6
            ),
            1.0
        )
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: false,
                effect: .off,
                strength: 0.6
            ),
            1.0
        )

        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: false,
                effect: .softSpotlight,
                strength: 0.6
            ),
            0.796,
            accuracy: 0.001
        )
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: false,
                effect: .cinematic,
                strength: 2.0
            ),
            0.62
        )
    }

    func testChromeApplierSkipsUnfocusedState() {
        let window = makeWindow()
        window.title = "Before"
        var appliedState: TerminalWindowChromeState?

        let state = TerminalWindowChromeState(
            title: "After",
            isFocused: false,
            background: TerminalRGBA(redByte: 1, greenByte: 2, blueByte: 3, alphaByte: 255),
            backgroundOpacity: 1.0,
            backgroundBlur: false
        )

        XCTAssertFalse(TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState))
        XCTAssertEqual(window.title, "Before")
        XCTAssertNil(appliedState)
    }

    func testChromeApplierAppliesFocusedTitleAndBackground() {
        let window = makeWindow()
        var appliedState: TerminalWindowChromeState?
        let background = TerminalRGBA(redByte: 12, greenByte: 34, blueByte: 56, alphaByte: 255)
        let state = TerminalWindowChromeState(
            title: "Focused Shell",
            isFocused: true,
            background: background,
            backgroundOpacity: 0.75,
            backgroundBlur: false
        )

        XCTAssertTrue(TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState))
        XCTAssertEqual(window.title, "Focused Shell")
        XCTAssertTrue(window.titlebarAppearsTransparent)
        XCTAssertFalse(window.isOpaque)
        assertColor(window.backgroundColor, matches: background, alpha: 0.75)
        XCTAssertEqual(appliedState, state)
    }

    func testChromeApplierKeepsOpaqueWindowForSolidBackground() {
        let window = makeWindow()
        var appliedState: TerminalWindowChromeState?
        let state = TerminalWindowChromeState(
            title: "Solid",
            isFocused: true,
            background: TerminalRGBA(redByte: 10, greenByte: 20, blueByte: 30, alphaByte: 255),
            backgroundOpacity: 1.0,
            backgroundBlur: false
        )

        XCTAssertTrue(TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState))
        XCTAssertTrue(window.isOpaque)
    }

    func testChromeApplierUsesTransparentWindowForBlurredBackground() {
        let window = makeWindow()
        var appliedState: TerminalWindowChromeState?
        let state = TerminalWindowChromeState(
            title: "Blurred",
            isFocused: true,
            background: TerminalRGBA(redByte: 10, greenByte: 20, blueByte: 30, alphaByte: 255),
            backgroundOpacity: 1.0,
            backgroundBlur: true
        )

        XCTAssertTrue(TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState))
        XCTAssertFalse(window.isOpaque)
    }

    func testChromeApplierRepairsWindowDriftForSameState() {
        let window = makeWindow()
        var appliedState: TerminalWindowChromeState?
        let background = TerminalRGBA(redByte: 12, greenByte: 34, blueByte: 56, alphaByte: 255)
        let state = TerminalWindowChromeState(
            title: "Focused Shell",
            isFocused: true,
            background: background,
            backgroundOpacity: 0.85,
            backgroundBlur: false
        )

        XCTAssertTrue(TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState))
        window.titlebarAppearsTransparent = false
        window.backgroundColor = .white
        window.isOpaque = true

        XCTAssertTrue(TerminalWindowChromeApplier.apply(state, to: window, appliedState: &appliedState))
        XCTAssertTrue(window.titlebarAppearsTransparent)
        XCTAssertFalse(window.isOpaque)
        assertColor(window.backgroundColor, matches: background, alpha: 0.85)
    }

    private func makeWindow() -> NSWindow {
        NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 200, height: 120),
            styleMask: [.titled],
            backing: .buffered,
            defer: true
        )
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
