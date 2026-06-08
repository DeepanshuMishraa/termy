import XCTest
@testable import TermySwift

final class TermyDeeplinkTests: XCTestCase {
    private func parse(_ string: String) -> TermyDeeplink? {
        guard let url = URL(string: string) else {
            return nil
        }
        return TermyDeeplink(url: url)
    }

    func testKnownDeeplinks() {
        XCTAssertEqual(parse("termy://new"), .newTab)
        XCTAssertEqual(parse("termy://settings"), .openSettings)
        XCTAssertEqual(parse("termy://config"), .openConfig)
        XCTAssertEqual(parse("termy://open/config"), .openConfig)
        XCTAssertEqual(parse("termy://store/theme-install?slug=dracula"), .installTheme(slug: "dracula"))
    }

    func testRejectsUnknownOrMalformed() {
        XCTAssertNil(parse("https://example.com"))
        XCTAssertNil(parse("termy://bogus"))
        XCTAssertNil(parse("termy://open/unknown"))
        XCTAssertNil(parse("termy://store/theme-install"))
        XCTAssertNil(parse("termy://store/theme-install?slug="))
    }
}
