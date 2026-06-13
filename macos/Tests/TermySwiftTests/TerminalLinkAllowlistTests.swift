import XCTest
@testable import TermySwift

/// Terminal output (including OSC 8 hyperlink targets) is attacker-influenced,
/// so only a narrow set of schemes may be handed to `NSWorkspace`. These tests
/// lock that allowlist down against regressions.
final class TerminalLinkAllowlistTests: XCTestCase {
    func testAllowsWebStyleSchemes() {
        for target in [
            "http://example.com",
            "https://example.com/path?q=1",
            "ftp://files.example.com/x",
            "mailto:user@example.com"
        ] {
            XCTAssertNotNil(
                TerminalFrameLink.openableURL(for: target),
                "expected \(target) to be openable"
            )
        }
    }

    func testRejectsDangerousSchemes() {
        for target in [
            "file:///Applications/Calculator.app",
            "javascript:alert(1)",
            "x-custom-handler://do-something",
            "vscode://file/etc/passwd",
            "smb://attacker/share"
        ] {
            XCTAssertNil(
                TerminalFrameLink.openableURL(for: target),
                "expected \(target) to be rejected"
            )
        }
    }

    func testRejectsSchemelessAndUnparseableTargets() {
        XCTAssertNil(TerminalFrameLink.openableURL(for: "example.com/no-scheme"))
        XCTAssertNil(TerminalFrameLink.openableURL(for: ""))
        XCTAssertNil(TerminalFrameLink.openableURL(for: "   "))
    }

    func testSchemeMatchIsCaseInsensitive() {
        XCTAssertNotNil(TerminalFrameLink.openableURL(for: "HTTPS://Example.com"))
        XCTAssertNil(TerminalFrameLink.openableURL(for: "FILE:///etc/hosts"))
    }
}
