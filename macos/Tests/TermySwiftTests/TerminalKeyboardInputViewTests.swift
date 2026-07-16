import AppKit
import XCTest
@testable import TermySwift

@MainActor
final class TerminalKeyboardInputViewTests: XCTestCase {
    func testWindowEventRoutesArrowThroughFirstResponderAndFFIEncoder() throws {
        let harness = AppKitEventHarness()
        let terminal = try LibTermyTerminal(displayCols: 80, rows: 24, loadUserConfig: false)
        var encodedBytes: [UInt8]?
        harness.inputView.onKeyInput = {
            encodedBytes = try? terminal.encodeKey($0)
        }

        harness.sendKeyDown(keyCode: 126, characters: "\u{F700}")

        XCTAssertEqual(encodedBytes, Array("\u{1b}[A".utf8))
    }

    func testWindowEventRoutesRepeatThroughKittyKeyboardFFIEncoder() throws {
        let harness = AppKitEventHarness()
        let terminal = try LibTermyTerminal(displayCols: 80, rows: 24, loadUserConfig: false)
        try terminal.feedOutput(Array("\u{1b}[>10u".utf8))
        var encodedBytes: [UInt8]?
        harness.inputView.onKeyInput = {
            encodedBytes = try? terminal.encodeKey($0)
        }

        harness.sendKeyDown(keyCode: 7, characters: "x", isRepeat: true)

        XCTAssertEqual(encodedBytes, Array("\u{1b}[120;1:2u".utf8))
    }

    func testWindowEventRoutesModifiedKeyThroughFirstResponder() {
        let harness = AppKitEventHarness()
        var sentKeys: [TerminalKeyInput] = []
        harness.inputView.onKeyInput = { sentKeys.append($0) }

        harness.sendKeyDown(
            keyCode: 126,
            characters: "\u{F700}",
            modifiers: [.option, .shift],
            isRepeat: true
        )

        XCTAssertEqual(sentKeys, [
            TerminalKeyInput(
                key: "up",
                alt: true,
                shift: true,
                eventKind: .repeat
            )
        ])
    }

    func testWindowEventsRoutePlainControlCommandFunctionNavigationAndKeypadKeys() {
        let harness = AppKitEventHarness()
        var sentKeys: [TerminalKeyInput] = []
        harness.inputView.onKeyInput = { sentKeys.append($0) }

        harness.sendKeyDown(keyCode: 7, characters: "x")
        harness.sendKeyDown(keyCode: 0, characters: "a", modifiers: [.control])
        harness.sendKeyDown(keyCode: 123, characters: "\u{F702}", modifiers: [.command])
        harness.sendKeyDown(keyCode: 122, characters: "\u{F704}")
        harness.sendKeyDown(keyCode: 115, characters: "\u{F729}")
        harness.sendKeyDown(keyCode: 121, characters: "\u{F72D}")
        harness.sendKeyDown(keyCode: 76, characters: "\r")

        XCTAssertEqual(sentKeys, [
            TerminalKeyInput(key: "x", keyChar: "x"),
            TerminalKeyInput(key: "a", keyChar: "a", control: true),
            TerminalKeyInput(key: "left", platform: true),
            TerminalKeyInput(key: "f1", function: true),
            TerminalKeyInput(key: "home"),
            TerminalKeyInput(key: "pagedown"),
            TerminalKeyInput(key: "enter")
        ])
    }

    func testWindowEventGivesSearchShortcutPrecedenceOverTerminalInput() {
        let harness = AppKitEventHarness()
        var showedSearch = false
        var sentKeys: [TerminalKeyInput] = []
        harness.inputView.onShowSearch = { showedSearch = true }
        harness.inputView.onKeyInput = { sentKeys.append($0) }

        harness.sendKeyDown(keyCode: 3, characters: "f", modifiers: [.command])

        XCTAssertTrue(showedSearch)
        XCTAssertTrue(sentKeys.isEmpty)
    }

    func testWindowEventDismissesSearchBeforeEncodingEscape() {
        let harness = AppKitEventHarness()
        var dismissedSearch = false
        var sentKeys: [TerminalKeyInput] = []
        harness.inputView.isSearchVisible = true
        harness.inputView.onDismissSearch = { dismissedSearch = true }
        harness.inputView.onKeyInput = { sentKeys.append($0) }

        harness.sendKeyDown(keyCode: 53, characters: "\u{1b}")

        XCTAssertTrue(dismissedSearch)
        XCTAssertTrue(sentKeys.isEmpty)
    }

    func testWindowEventsRouteMousePressDragAndRelease() {
        let harness = AppKitEventHarness()
        harness.inputView.cols = 80
        harness.inputView.rows = 24
        var sentMouse: [TerminalMouseInput] = []
        harness.inputView.onMouseInput = {
            sentMouse.append($0)
            return true
        }

        harness.sendMouse(
            type: .leftMouseDown,
            at: harness.point(col: 3, row: 2),
            modifiers: [.option, .shift]
        )
        harness.sendMouse(
            type: .leftMouseDragged,
            at: harness.point(col: 4, row: 3),
            modifiers: [.option, .shift]
        )
        harness.sendMouse(
            type: .leftMouseUp,
            at: harness.point(col: 4, row: 3),
            modifiers: [.option, .shift]
        )

        XCTAssertEqual(sentMouse, [
            TerminalMouseInput(
                kind: .press,
                button: .left,
                position: TerminalGridPosition(col: 3, row: 2),
                control: false,
                alt: true,
                shift: true
            ),
            TerminalMouseInput(
                kind: .drag,
                button: .left,
                position: TerminalGridPosition(col: 4, row: 3),
                control: false,
                alt: true,
                shift: true
            ),
            TerminalMouseInput(
                kind: .release,
                button: .left,
                position: TerminalGridPosition(col: 4, row: 3),
                control: false,
                alt: true,
                shift: true
            )
        ])
    }

    func testWindowEventsRouteMouseReportingThroughFFIEncoder() throws {
        let harness = AppKitEventHarness()
        harness.inputView.cols = 80
        harness.inputView.rows = 24
        let terminal = try LibTermyTerminal(displayCols: 80, rows: 24, loadUserConfig: false)
        try terminal.feedOutput(Array("\u{1b}[?1002h\u{1b}[?1006h".utf8))
        var encoded: [[UInt8]] = []
        harness.inputView.onMouseInput = {
            guard let bytes = try? terminal.encodeMouse($0) else {
                return false
            }
            encoded.append(bytes)
            return true
        }

        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 3, row: 2))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 3, row: 2))

        XCTAssertEqual(encoded, [
            Array("\u{1b}[<0;4;3M".utf8),
            Array("\u{1b}[<0;4;3m".utf8)
        ])
    }

    func testWindowEventsRouteLineScrollToScrollback() {
        let harness = AppKitEventHarness()
        var scrollDeltas: [Int] = []
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onScrollLines = { scrollDeltas.append($0) }

        harness.sendScroll(deltaY: 2, precise: false)

        XCTAssertEqual(scrollDeltas, [6])
    }

    func testWindowEventsAccumulatePreciseTrackpadScroll() {
        let harness = AppKitEventHarness()
        var scrollDeltas: [Int] = []
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onScrollLines = { scrollDeltas.append($0) }

        harness.sendScroll(deltaY: 4, precise: true)
        harness.sendScroll(deltaY: 4, precise: true)

        XCTAssertEqual(scrollDeltas, [1])
    }

    func testWindowEventsFallBackToDragSelectionWhenMouseReportingIsDisabled() {
        let harness = AppKitEventHarness()
        harness.inputView.cols = 80
        harness.inputView.rows = 24
        var selections: [TerminalSelection?] = []
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectionChanged = { selections.append($0) }

        harness.sendMouse(type: .leftMouseDown, at: harness.point(col: 2, row: 1))
        harness.sendMouse(type: .leftMouseDragged, at: harness.point(col: 8, row: 4))
        harness.sendMouse(type: .leftMouseUp, at: harness.point(col: 8, row: 4))

        let selection = TerminalSelection(
            anchor: TerminalGridPosition(col: 2, row: 1),
            active: TerminalGridPosition(col: 8, row: 4)
        )
        XCTAssertEqual(selections.compactMap { $0 }, [selection, selection])
    }

    func testLeftClickSelectsKittyGraphicsBeforeStartingTextSelection() {
        let harness = AppKitEventHarness()
        harness.inputView.cols = 80
        harness.inputView.rows = 24
        var selectedPoint: CGPoint?
        var selections: [TerminalSelection?] = []
        harness.inputView.onMouseInput = { _ in false }
        harness.inputView.onSelectKittyGraphics = {
            selectedPoint = $0
            return true
        }
        harness.inputView.onSelectionChanged = { selections.append($0) }

        let clickPoint = harness.point(col: 3, row: 2)
        harness.sendMouse(type: .leftMouseDown, at: clickPoint)
        harness.sendMouse(type: .leftMouseUp, at: clickPoint)

        let config = harness.inputView.renderConfig
        XCTAssertEqual(
            selectedPoint?.x ?? -1,
            config.paddingX + 3.5 * config.cellWidth,
            accuracy: 0.001
        )
        XCTAssertEqual(
            selectedPoint?.y ?? -1,
            config.paddingY + 2.5 * config.cellHeight,
            accuracy: 0.001
        )
        XCTAssertTrue(selections.isEmpty)
    }

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
