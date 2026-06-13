import XCTest
@testable import TermySwift

/// Covers the Swift keyCode → terminal-key-name mapping (the event→FFI input
/// path) that hands off to the Rust encoder.
final class TerminalKeyInputMappingTests: XCTestCase {
    func testSpecialKeysMapToNames() {
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 36)?.key, "enter")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 48)?.key, "tab")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 51)?.key, "backspace")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 53)?.key, "escape")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 123)?.key, "left")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 124)?.key, "right")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 125)?.key, "down")
        XCTAssertEqual(KeyboardCaptureView.specialKey(for: 126)?.key, "up")
    }

    func testFunctionKeysCarryFunctionFlag() {
        let f1 = KeyboardCaptureView.specialKey(for: 122)
        XCTAssertEqual(f1?.key, "f1")
        XCTAssertEqual(f1?.function, true)
        XCTAssertEqual(f1?.usesCharacter, false)
    }

    func testSpaceCarriesTypedCharacter() {
        let space = KeyboardCaptureView.specialKey(for: 49)
        XCTAssertEqual(space?.key, "space")
        XCTAssertEqual(space?.usesCharacter, true)
    }

    func testNonSpecialKeyFallsThroughToCharacter() {
        // keyCode 0 ('a' on US layouts) is not in the special table.
        XCTAssertNil(KeyboardCaptureView.specialKey(for: 0))
    }
}
