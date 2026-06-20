import AppKit
import XCTest
@testable import TermySwift

final class NativeKeyEventClassifierTests: XCTestCase {
    func testCommandDigitUsesPhysicalNumberRowKeyCodeForTabSwitching() {
        let event = Self.keyEvent(
            keyCode: 18,
            characters: "!",
            modifiers: [.command]
        )

        XCTAssertEqual(
            NativeKeyEventClassifier.canonicalTriggers(for: event),
            ["cmd-1", "secondary-1"]
        )
    }

    func testCommandDigitUsesKeypadKeyCodeForTabSwitching() {
        let event = Self.keyEvent(
            keyCode: 83,
            characters: "x",
            modifiers: [.command]
        )

        XCTAssertEqual(
            NativeKeyEventClassifier.canonicalTriggers(for: event),
            ["cmd-1", "secondary-1"]
        )
    }

    func testCommandArrowCreatesPlatformLineEditingInput() {
        let left = NativeKeyEventClassifier.terminalLineEditingInput(for: Self.keyEvent(
            keyCode: 123,
            characters: "\u{F702}",
            modifiers: [.command]
        ))
        let right = NativeKeyEventClassifier.terminalLineEditingInput(for: Self.keyEvent(
            keyCode: 124,
            characters: "\u{F703}",
            modifiers: [.command],
            isRepeat: true
        ))

        XCTAssertEqual(left, TerminalKeyInput(key: "left", platform: true))
        XCTAssertEqual(right, TerminalKeyInput(key: "right", platform: true, eventKind: .repeat))
    }

    func testModifiedCommandArrowDoesNotCreateLineEditingInput() {
        let event = Self.keyEvent(
            keyCode: 123,
            characters: "\u{F702}",
            modifiers: [.command, .shift]
        )

        XCTAssertNil(NativeKeyEventClassifier.terminalLineEditingInput(for: event))
    }

    func testPlainArrowDoesNotCreateLineEditingInput() {
        let event = Self.keyEvent(keyCode: 123, characters: "\u{F702}")

        XCTAssertNil(NativeKeyEventClassifier.terminalLineEditingInput(for: event))
    }

    private static func keyEvent(
        keyCode: UInt16,
        characters: String,
        modifiers: NSEvent.ModifierFlags = [],
        isRepeat: Bool = false
    ) -> NSEvent {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: modifiers,
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: characters,
            charactersIgnoringModifiers: characters,
            isARepeat: isRepeat,
            keyCode: keyCode
        )!
    }
}
