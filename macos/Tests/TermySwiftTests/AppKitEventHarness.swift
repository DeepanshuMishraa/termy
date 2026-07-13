import AppKit
@testable import TermySwift

@MainActor
final class AppKitEventHarness {
    let window: NSWindow
    let inputView: KeyboardCaptureView

    init(size: NSSize = NSSize(width: 800, height: 480)) {
        window = NSWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        inputView = KeyboardCaptureView(frame: NSRect(origin: .zero, size: size))
        window.contentView = inputView
        window.makeKeyAndOrderFront(nil)
        precondition(window.makeFirstResponder(inputView))
    }

    func sendKeyDown(
        keyCode: UInt16,
        characters: String,
        modifiers: NSEvent.ModifierFlags = [],
        isRepeat: Bool = false
    ) {
        let event = NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: modifiers,
            timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber,
            context: nil,
            characters: characters,
            charactersIgnoringModifiers: characters,
            isARepeat: isRepeat,
            keyCode: keyCode
        )!
        window.sendEvent(event)
    }

    func sendMouse(
        type: NSEvent.EventType,
        at location: NSPoint,
        modifiers: NSEvent.ModifierFlags = [],
        clickCount: Int = 1
    ) {
        let event = NSEvent.mouseEvent(
            with: type,
            location: location,
            modifierFlags: modifiers,
            timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber,
            context: nil,
            eventNumber: 0,
            clickCount: clickCount,
            pressure: type == .leftMouseUp ? 0 : 1
        )!
        window.sendEvent(event)
    }

    func sendScroll(
        deltaX: Int32 = 0,
        deltaY: Int32,
        precise: Bool,
        at location: NSPoint? = nil
    ) {
        let event = CGEvent(
            scrollWheelEvent2Source: nil,
            units: precise ? .pixel : .line,
            wheelCount: 2,
            wheel1: deltaY,
            wheel2: deltaX,
            wheel3: 0
        )!
        let location = location ?? point(col: 0, row: 0)
        event.location = window.convertPoint(toScreen: location)
        // CGEvent does not retain the synthetic NSWindow association needed by
        // NSWindow.sendEvent, so deliver the real NSEvent to the AppKit scroll
        // override directly.
        inputView.scrollWheel(with: NSEvent(cgEvent: event)!)
    }

    func point(col: Int, row: Int) -> NSPoint {
        let config = inputView.renderConfig
        return NSPoint(
            x: config.paddingX + (CGFloat(col) + 0.5) * config.cellWidth,
            y: inputView.bounds.height - config.paddingY - (CGFloat(row) + 0.5) * config.cellHeight
        )
    }
}
