import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class TerminalKeyboardInputViewTests: XCTestCase {
    func testDragSelectionIgnoresSameCellJitter() {
        let position = TerminalGridPosition(col: 12, row: 4)

        XCTAssertNil(KeyboardCaptureView.dragSelection(anchor: position, active: position))
    }

    func testDragSelectionStartsAfterMovingToAnotherCell() {
        let anchor = TerminalGridPosition(col: 12, row: 4)
        let active = TerminalGridPosition(col: 13, row: 4)

        XCTAssertEqual(
            KeyboardCaptureView.dragSelection(anchor: anchor, active: active),
            TerminalSelection(anchor: anchor, active: active)
        )
    }

    func testEscapeDismissesVisibleSearchInsteadOfSendingTerminalKey() {
        let view = KeyboardCaptureView()
        var dismissedSearch = false
        var sentKeys: [TerminalKeyInput] = []
        view.isInputEnabled = true
        view.isSearchVisible = true
        view.onDismissSearch = {
            dismissedSearch = true
        }
        view.onKeyInput = {
            sentKeys.append($0)
        }

        view.keyDown(with: Self.escapeEvent())

        XCTAssertTrue(dismissedSearch)
        XCTAssertTrue(sentKeys.isEmpty)
    }

    func testEscapeStillSendsTerminalKeyWhenSearchIsHidden() {
        let view = KeyboardCaptureView()
        var dismissedSearch = false
        var sentKeys: [TerminalKeyInput] = []
        view.isInputEnabled = true
        view.isSearchVisible = false
        view.onDismissSearch = {
            dismissedSearch = true
        }
        view.onKeyInput = {
            sentKeys.append($0)
        }

        view.keyDown(with: Self.escapeEvent())

        XCTAssertFalse(dismissedSearch)
        XCTAssertEqual(sentKeys.map(\.key), ["escape"])
    }

    func testArrowKeysSendTerminalInput() {
        let view = KeyboardCaptureView()
        var sentKeys: [TerminalKeyInput] = []
        view.isInputEnabled = true
        view.onKeyInput = {
            sentKeys.append($0)
        }

        view.keyDown(with: Self.keyEvent(keyCode: 125, characters: "\u{F701}"))
        view.keyDown(with: Self.keyEvent(keyCode: 126, characters: "\u{F700}"))

        XCTAssertEqual(sentKeys.map(\.key), ["down", "up"])
    }

    func testSearchVisibleInputDisabledStillReceivesHitForFocusRestore() {
        let view = KeyboardCaptureView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        view.isInputEnabled = false
        view.isSearchVisible = true

        XCTAssertIdentical(view.hitTest(NSPoint(x: 50, y: 50)), view)
    }

    func testInputDisabledWithoutSearchIgnoresHit() {
        let view = KeyboardCaptureView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        view.isInputEnabled = false
        view.isSearchVisible = false

        XCTAssertNil(view.hitTest(NSPoint(x: 50, y: 50)))
    }

    func testDropInputRejectsTerminalControlCharactersInPaths() {
        XCTAssertFalse(TerminalDropInput.containsTerminalControlCharacter("/tmp/safe file's name.txt"))
        XCTAssertTrue(TerminalDropInput.containsTerminalControlCharacter("/tmp/\u{15}open -a Calculator\n"))
        XCTAssertTrue(TerminalDropInput.containsTerminalControlCharacter("/tmp/carriage\rreturn"))
        XCTAssertTrue(TerminalDropInput.containsTerminalControlCharacter("/tmp/delete\u{7f}char"))
    }

    private static func escapeEvent() -> NSEvent {
        keyEvent(keyCode: 53, characters: "\u{1b}")
    }

    private static func keyEvent(keyCode: UInt16, characters: String) -> NSEvent {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: [],
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: characters,
            charactersIgnoringModifiers: characters,
            isARepeat: false,
            keyCode: keyCode
        )!
    }
}
