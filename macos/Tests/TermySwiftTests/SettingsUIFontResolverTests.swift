import XCTest
@testable import TermySwift

final class SettingsUIFontResolverTests: XCTestCase {
    func testUnsetUsesSystemFont() {
        let decision = SettingsUIFontResolver.font(
            family: "JetBrains Mono",
            isExplicitlySet: false,
            size: 13
        )
        XCTAssertEqual(decision, .system(size: 13))
    }

    func testExplicitlySetUsesCustomFont() {
        let decision = SettingsUIFontResolver.font(
            family: "Avenir Next",
            isExplicitlySet: true,
            size: 13
        )
        XCTAssertEqual(decision, .custom(name: "Avenir Next", size: 13))
    }

    func testWhitespaceFamilyFallsBackToSystem() {
        let decision = SettingsUIFontResolver.font(
            family: "   ",
            isExplicitlySet: true,
            size: 13
        )
        XCTAssertEqual(decision, .system(size: 13))
    }

    /// The flag, not the value, drives the resolver: an explicitly set default
    /// family still resolves to custom once the upstream flag is true.
    func testExplicitDefaultFamilyTreatedAsCustomWhenFlagged() {
        let decision = SettingsUIFontResolver.font(
            family: "JetBrains Mono",
            isExplicitlySet: true,
            size: 13
        )
        XCTAssertEqual(decision, .custom(name: "JetBrains Mono", size: 13))
    }
}
