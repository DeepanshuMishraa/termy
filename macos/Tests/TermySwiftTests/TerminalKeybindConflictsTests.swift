import XCTest
@testable import TermySwift

final class TerminalKeybindConflictsTests: XCTestCase {
    private func keybind(_ trigger: String, _ action: String) -> TermyKeybindConfiguration {
        TermyKeybindConfiguration(trigger: trigger, action: action)
    }

    func testNoConflictsWhenTriggersUnique() {
        let conflicts = TerminalKeybindConflicts.conflictingTriggers(in: [
            keybind("cmd-t", "new_tab"),
            keybind("cmd-w", "close_tab"),
        ])
        XCTAssertTrue(conflicts.isEmpty)
    }

    func testExactDuplicateTriggerConflicts() {
        let conflicts = TerminalKeybindConflicts.conflictingTriggers(in: [
            keybind("cmd-k", "clear_buffer"),
            keybind("cmd-k", "toggle_command_palette"),
        ])
        XCTAssertEqual(conflicts, ["cmd-k"])
    }

    func testModifierOrderAndSecondaryAliasNormalize() {
        let conflicts = TerminalKeybindConflicts.conflictingTriggers(in: [
            keybind("shift-cmd-d", "split_pane_horizontal"),
            keybind("cmd-shift-d", "split_pane_vertical"),
            keybind("secondary-k", "clear_buffer"),
            keybind("cmd-k", "toggle_command_palette"),
        ])
        XCTAssertEqual(conflicts, ["shift-cmd-d", "cmd-shift-d", "secondary-k", "cmd-k"])
    }
}
