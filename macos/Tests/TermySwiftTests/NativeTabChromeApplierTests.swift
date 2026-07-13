import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class NativeTabChromeApplierTests: XCTestCase {
    private func makeBareWindow() -> NSWindow {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: true
        )
        window.isReleasedWhenClosed = false
        return window
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
        // Translucent background: both tab windows get a clear backing and a
        // transparent window (the SwiftUI surface paints the tint once).
        assertClearBackground(first.backgroundColor)
        assertClearBackground(second.backgroundColor)
        XCTAssertFalse(first.isOpaque)
        XCTAssertFalse(second.isOpaque)
        XCTAssertEqual(first.title, "First")
        XCTAssertEqual(second.title, "Active Second")
    }

    func testNativeTabDescriptorsKeepAppKitTabGroupOrderWhenSourceIsSecondTab() {
        let manager = NativeTabWindowManager.shared
        let first = makeBareWindow()
        let second = makeBareWindow()
        manager.configure(first)
        manager.configure(second)
        first.title = "First"
        second.title = "Second"
        first.addTabbedWindow(second, ordered: .above)

        let descriptors = manager.tabDescriptors(for: second)

        XCTAssertEqual(descriptors.map(\.title), ["First", "Second"])
        XCTAssertEqual(descriptors.map(\.index), [0, 1])
    }

    func testStoreScopedChromeRefreshUpdatesInactiveTabWindowTitle() {
        let manager = NativeTabWindowManager.shared
        let window = makeBareWindow()
        let store = TerminalWorkspaceStore()
        manager.configure(window)
        TerminalCommandRouter.shared.register(store, for: window)
        defer {
            TerminalCommandRouter.shared.unregister(window: window)
        }

        XCTAssertTrue(store.focusedTerminal?.applyTerminalTitle("background build") ?? false)
        XCTAssertTrue(manager.applyFocusedTerminalChrome(for: store))

        XCTAssertEqual(window.title, "background build")
    }

    func testNativeTabLifecycleRoutesRenamePinReorderSelectAndCloseThroughAppKitGroup() {
        let manager = NativeTabWindowManager.shared
        let windows = (0..<3).map { _ in makeBareWindow() }
        let stores = windows.map { window in
            let store = TerminalWorkspaceStore()
            manager.configure(window)
            TerminalCommandRouter.shared.register(store, for: window)
            return store
        }
        defer {
            for (window, store) in zip(windows, stores) {
                TerminalCommandRouter.shared.unregister(window: window)
                store.suspendRefresh()
                window.close()
            }
        }

        windows[0].addTabbedWindow(windows[1], ordered: .above)
        windows[1].addTabbedWindow(windows[2], ordered: .above)
        windows[0].makeKeyAndOrderFront(nil)

        var descriptors = manager.tabDescriptors(for: windows[0])
        XCTAssertEqual(descriptors.map(\.id), windows.map(ObjectIdentifier.init))

        let second = descriptors[1]
        manager.renameNativeTab(second, title: "build")
        manager.setNativeTabPinned(second, pinned: true)

        descriptors = manager.tabDescriptors(for: windows[0])
        XCTAssertEqual(descriptors[1].title, "build")
        XCTAssertTrue(descriptors[1].hasManualTitle)
        XCTAssertTrue(descriptors[1].isPinned)
        XCTAssertEqual(windows[1].title, "build")

        manager.moveNativeTab(descriptors[2], toIndex: 0)
        descriptors = manager.tabDescriptors(for: windows[2])
        XCTAssertEqual(descriptors.map(\.id), [
            ObjectIdentifier(windows[2]),
            ObjectIdentifier(windows[0]),
            ObjectIdentifier(windows[1]),
        ])

        let buildTab = descriptors[2]
        manager.selectNativeTab(buildTab)
        XCTAssertTrue(windows[1].tabGroup?.selectedWindow === windows[1])

        manager.closeNativeTab(buildTab)
        RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        XCTAssertFalse(windows[1].isVisible)
        XCTAssertEqual(manager.tabDescriptors(for: windows[0]).map(\.id), [
            ObjectIdentifier(windows[2]),
            ObjectIdentifier(windows[0]),
        ])
    }

    private func assertClearBackground(
        _ color: NSColor,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let color = color.usingColorSpace(.sRGB) ?? color
        XCTAssertEqual(color.alphaComponent, 0.0, accuracy: 0.001, file: file, line: line)
    }
}
