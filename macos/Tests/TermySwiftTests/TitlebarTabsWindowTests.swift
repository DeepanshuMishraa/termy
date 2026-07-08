import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class TitlebarTabsWindowTests: XCTestCase {
    private func makeTitlebarTabsWindow(title: String) -> TitlebarTabsWindow {
        let window = TitlebarTabsWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.titleVisibility = .hidden
        window.titlebarTabs = true
        return window
    }

    private func pumpRunLoop(until timeout: TimeInterval = 0.5) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
    }

    func testTabBarRePinsAfterSwitchingBackToFirstTab() throws {
        let first = makeTitlebarTabsWindow(title: "First")
        let second = makeTitlebarTabsWindow(title: "Second")
        first.tabbingIdentifier = "termy.test.titlebar-tabs"
        second.tabbingIdentifier = "termy.test.titlebar-tabs"
        first.addTabbedWindow(second, ordered: .above)

        first.makeKeyAndOrderFront(nil)
        pumpRunLoop()

        second.makeKeyAndOrderFront(nil)
        pumpRunLoop()

        first.makeKeyAndOrderFront(nil)
        pumpRunLoop()

        guard let tabBarView = first.titlebarView?.firstDescendant(withClassName: "NSTabBar"),
              let clipView = tabBarView.firstSuperview(withClassName: "NSTitlebarAccessoryClipView")
                ?? tabBarView.firstSuperview(withClassName: "NSTitlebarAccessoryContainerView"),
              let toolbarView = first.titlebarView?.firstDescendant(withClassName: "NSToolbarView")
        else {
            throw XCTSkip("Native tab bar views are unavailable in this test environment")
        }

        let clipFrameInToolbar = clipView.convert(clipView.bounds, to: toolbarView)
        XCTAssertLessThan(abs(clipFrameInToolbar.minY - 2), 4,
                          "tab bar clip view should stay pinned near the toolbar top after switching back")
    }
}

private extension NSView {
    func firstDescendant(withClassName name: String) -> NSView? {
        for subview in subviews {
            if subview.className == name {
                return subview
            }
            if let found = subview.firstDescendant(withClassName: name) {
                return found
            }
        }
        return nil
    }

    func firstSuperview(withClassName name: String) -> NSView? {
        guard let superview else { return nil }
        if superview.className == name {
            return superview
        }
        return superview.firstSuperview(withClassName: name)
    }
}

private extension NSWindow {
    var titlebarView: NSView? {
        guard let themeFrame = contentView?.superview?.superview,
              themeFrame.responds(to: Selector(("titlebarView")))
        else {
            return nil
        }
        return themeFrame.value(forKey: "titlebarView") as? NSView
    }
}
