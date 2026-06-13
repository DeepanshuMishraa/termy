import XCTest
@testable import TermySwift

final class TermyThemeStoreTests: XCTestCase {
    func testParsesRegistryIndex() throws {
        let json = """
        {
          "version": 1,
          "themes": [
            { "name": "Dracula", "slug": "dracula", "description": "Dark vampire theme", "latestVersion": "1.2.0", "file": "dracula.json" },
            { "name": "Gruvbox", "slug": "gruvbox", "latestVersion": "0.3.0", "file": "gruvbox.json" }
          ]
        }
        """
        let entries = try TermyThemeStore.parseIndex(Data(json.utf8))
        XCTAssertEqual(entries.map(\.slug), ["dracula", "gruvbox"])
        XCTAssertEqual(entries[0].name, "Dracula")
        XCTAssertEqual(entries[0].description, "Dark vampire theme")
        XCTAssertEqual(entries[0].latestVersion, "1.2.0")
        // `description` is optional and tolerated when missing.
        XCTAssertNil(entries[1].description)
    }

    func testRejectsMalformedIndex() {
        XCTAssertThrowsError(try TermyThemeStore.parseIndex(Data("not json".utf8)))
    }
}
