import XCTest
@testable import TermySwift

@MainActor
final class TerminalSearchInteractionTests: XCTestCase {
    func testSearchOptionsAndNavigationWrapThroughRealTerminal() throws {
        let terminal = try LibTermyTerminal(displayCols: 40, rows: 6, loadUserConfig: false)
        try terminal.feedOutput(Array("Alpha alpha ALPHA\r\nbeta-1 beta-2\r\nalpha".utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()

        viewModel.updateSearch("alpha")
        XCTAssertEqual(viewModel.searchMatches.count, 4)
        XCTAssertEqual(viewModel.activeSearchMatchIndex, 0)

        viewModel.selectPreviousSearchMatch()
        XCTAssertEqual(viewModel.activeSearchMatchIndex, 3)
        viewModel.selectNextSearchMatch()
        XCTAssertEqual(viewModel.activeSearchMatchIndex, 0)

        viewModel.updateSearch("alpha", options: TerminalSearchOptions(caseSensitive: true))
        XCTAssertEqual(viewModel.searchMatches.count, 2)

        viewModel.updateSearch("beta-[12]", options: TerminalSearchOptions(usesRegex: true))
        XCTAssertEqual(viewModel.searchMatches.count, 2)
    }

    func testOpenSearchRefreshesWhenOutputArrives() async throws {
        let terminal = try LibTermyTerminal(displayCols: 40, rows: 6, loadUserConfig: false)
        try terminal.feedOutput(Array("needle".utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()
        viewModel.updateSearch("needle")
        XCTAssertEqual(viewModel.searchMatches.count, 1)

        try terminal.feedOutput(Array("\r\nsecond needle\r\n".utf8))
        XCTAssertEqual(try terminal.search("needle", options: TerminalSearchOptions()).count, 2)
        try await Task.sleep(for: .milliseconds(200))
        viewModel.refreshExternalOutput()

        XCTAssertEqual(viewModel.searchMatches.count, 2)
    }

    func testSearchFindsMatchesAcrossScrollback() throws {
        let terminal = try LibTermyTerminal(displayCols: 40, rows: 4, loadUserConfig: false)
        let output = (0..<20)
            .map { index in index == 1 || index == 18 ? "line \(index) needle" : "line \(index)" }
            .joined(separator: "\r\n")
        try terminal.feedOutput(Array(output.utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()

        viewModel.updateSearch("needle")

        XCTAssertEqual(viewModel.searchMatches.count, 2)
        XCTAssertGreaterThan(viewModel.searchMatches[1].row, viewModel.searchMatches[0].row)
        XCTAssertGreaterThan(viewModel.frame.historySize, 0)
    }

    func testResizePreservesOpenSearchAndActiveMatch() async throws {
        let terminal = try LibTermyTerminal(displayCols: 40, rows: 6, loadUserConfig: false)
        try terminal.feedOutput(Array("first needle and enough text to reflow\r\nsecond needle".utf8))
        let viewModel = TerminalViewModel(displayTerminal: terminal)
        viewModel.start()
        defer { viewModel.stop() }
        viewModel.refreshExternalOutput()
        viewModel.updateSearch("needle")
        viewModel.selectNextSearchMatch()
        XCTAssertEqual(viewModel.activeSearchMatchIndex, 1)

        viewModel.resize(cols: 16, rows: 6, cellWidth: 9, cellHeight: 20)
        try await Task.sleep(for: .milliseconds(50))
        viewModel.refreshExternalOutput()

        XCTAssertEqual(viewModel.searchMatches.count, 2)
        XCTAssertEqual(viewModel.activeSearchMatchIndex, 1)
    }
}
