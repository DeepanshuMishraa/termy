import XCTest
@testable import TermySwift

final class TerminalKeybindActionTests: XCTestCase {
    func testCanonicalIdentifiersRoundTrip() {
        let identifiers = [
            "app_info", "restart_app", "open_config", "prettify_config",
            "toggle_tab_bar_visibility", "move_tab_left", "move_tab_right",
            "switch_tab_left", "switch_tab_right", "new_tab", "close_tab",
            "close_pane_or_tab", "close_pane", "minimize_window", "quit",
            "toggle_command_palette", "split_pane_vertical", "split_pane_horizontal",
            "focus_pane_next", "focus_pane_previous", "focus_pane_left",
            "focus_pane_right", "focus_pane_up", "focus_pane_down", "resize_pane_left",
            "resize_pane_right", "resize_pane_up", "resize_pane_down", "toggle_pane_zoom",
            "increase_font_size", "decrease_font_size", "reset_font_size", "copy",
            "paste", "open_search", "close_search", "search_next", "search_previous",
            "toggle_search_case_sensitive", "toggle_search_regex", "clear_buffer",
            "send_interrupt", "run_task",
        ]
        for identifier in identifiers {
            XCTAssertEqual(
                TerminalKeybindAction(identifier: identifier).identifier,
                identifier,
                "identifier \(identifier) should round-trip"
            )
        }
    }

    func testCycleTabsIsAliasForSwitchTabRight() {
        XCTAssertEqual(TerminalKeybindAction(identifier: "cycle_tabs"), .switchTabRight)
    }

    func testSwitchToTabParsesIndex() {
        XCTAssertEqual(TerminalKeybindAction(identifier: "switch_to_tab_1"), .switchToTab(1))
        XCTAssertEqual(TerminalKeybindAction(identifier: "switch_to_tab_9"), .switchToTab(9))
        XCTAssertEqual(TerminalKeybindAction(identifier: "switch_to_tab_12").identifier, "switch_to_tab_12")
    }

    func testUnknownActionIsPreserved() {
        XCTAssertEqual(TerminalKeybindAction(identifier: "future_action"), .unknown("future_action"))
        XCTAssertEqual(TerminalKeybindAction(identifier: "switch_to_tab_0"), .unknown("switch_to_tab_0"))
    }

    func testDirectionalActionsMapToDirections() {
        XCTAssertEqual(TerminalKeybindAction(identifier: "focus_pane_up"), .focusPane(.up))
        XCTAssertEqual(TerminalKeybindAction(identifier: "resize_pane_down"), .resizePane(.down))
    }
}
