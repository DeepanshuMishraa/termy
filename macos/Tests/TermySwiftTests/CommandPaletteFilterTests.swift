import XCTest
@testable import TermySwift

final class CommandPaletteFilterTests: XCTestCase {
    private func score(_ query: String, _ title: String, action: String = "") -> Int? {
        CommandPaletteFilter.match(query: query, title: title, action: action)?.score
    }

    func testEmptyQueryMatchesEverything() {
        let match = CommandPaletteFilter.match(query: "", title: "New Tab", action: "new_tab")
        XCTAssertEqual(match, CommandPaletteMatch(score: 0, matchedTitleIndices: []))
    }

    func testInOrderSubsequenceMatchesAndOutOfOrderDoesNot() {
        XCTAssertNotNil(score("spr", "Split Right"))
        XCTAssertNil(score("rps", "Split Right"))
        XCTAssertNil(score("xyz", "Split Right"))
    }

    func testPrefixOutranksScatteredMatch() {
        let prefix = score("new", "New Tab")!
        let scattered = score("new", "Next Pane Wide")!
        XCTAssertGreaterThan(prefix, scattered)
    }

    func testWordStartsOutrankMidWordHits() {
        // "nt" hits both word starts in "New Tab" but mid-word letters in
        // "Increase Font Size" — the acronym match must rank first.
        let acronym = score("nt", "New Tab")!
        let midWord = score("nt", "Increase Font Size")!
        XCTAssertGreaterThan(acronym, midWord)
    }

    func testConsecutiveRunOutranksGaps() {
        let consecutive = score("split", "Split Right")!
        let gappy = score("split", "Set Pane Layout Right")!
        XCTAssertGreaterThan(consecutive, gappy)
    }

    func testActionIdFallbackMatchesWithLowScore() {
        let match = CommandPaletteFilter.match(query: "search", title: "Find", action: "open_search")
        XCTAssertEqual(match, CommandPaletteMatch(score: 1, matchedTitleIndices: []))

        let titleMatch = CommandPaletteFilter.match(query: "find", title: "Find", action: "open_search")!
        XCTAssertGreaterThan(titleMatch.score, match!.score)
    }

    func testMatchedIndicesPointAtTitleCharacters() {
        let match = CommandPaletteFilter.match(query: "tab", title: "New Tab", action: "new_tab")!
        XCTAssertEqual(match.matchedTitleIndices, [4, 5, 6])
    }

    func testMatchingIsCaseInsensitive() {
        XCTAssertNotNil(score("NEW TAB", "New Tab"))
        XCTAssertNotNil(score("new tab", "NEW TAB"))
    }
}
