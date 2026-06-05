import XCTest

@testable import TermySwift

final class TerminalHyperlinkTests: XCTestCase {
    func testOsc8HyperlinkIsReportedUnderLinkCells() throws {
        let cols = 48
        let rows = 8
        let terminal = try LibTermyTerminal(
            cols: UInt16(cols),
            rows: UInt16(rows),
            loadUserConfig: false
        )

        // printf expands \033 to ESC; emits "docs" wrapped in an OSC 8 link.
        // The echoed command line only contains literal backslash text, so the
        // sole hyperlinked cells are printf's four "docs" output cells.
        let command = "printf '\\033]8;;https://example.com\\033\\\\docs\\033]8;;\\033\\\\'\n"
        try terminal.write(Array(command.utf8))

        var link: TerminalFrameLink?
        for _ in 0..<100 {
            link = firstHyperlink(in: terminal, cols: cols, rows: rows)
            if link != nil {
                break
            }
            usleep(20_000)
        }

        let found = try XCTUnwrap(link, "OSC 8 output should surface a hyperlink")
        XCTAssertEqual(found.target, "https://example.com")
        XCTAssertEqual(found.endCol - found.startCol + 1, 4, "link run should cover 'docs'")

        // Cells outside the run never report a hyperlink.
        XCTAssertNil(terminal.hyperlink(atRow: rows - 1, col: cols - 1))
    }

    private func firstHyperlink(
        in terminal: LibTermyTerminal,
        cols: Int,
        rows: Int
    ) -> TerminalFrameLink? {
        for row in 0..<rows {
            for col in 0..<cols {
                if let link = terminal.hyperlink(atRow: row, col: col) {
                    return link
                }
            }
        }
        return nil
    }
}
