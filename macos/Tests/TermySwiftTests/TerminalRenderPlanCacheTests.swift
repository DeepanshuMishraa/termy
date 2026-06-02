import XCTest
@testable import TermySwift

final class TerminalRenderPlanCacheTests: XCTestCase {
    private let config = TerminalRenderConfig.default

    private func frame(_ text: String, cols: Int = 3, rows: Int = 3) -> TerminalFrame {
        TerminalFrame.plainTextPreview(text, cols: cols, rows: rows)
    }

    func testInitialUpdateIsFullRebuild() {
        let cache = TerminalRenderPlanCache()
        cache.update(frame: frame("aaa\nbbb\nccc"), renderConfig: config, damage: .partial([]))

        XCTAssertTrue(cache.stats.wasFullRebuild)
        XCTAssertEqual(cache.stats.rebuiltRowCount, 3)
        XCTAssertEqual(cache.stats.totalRowCount, 3)
    }

    func testPartialRebuildTouchesOnlyDamagedRows() {
        let cache = TerminalRenderPlanCache()
        cache.update(frame: frame("aaa\nbbb\nccc"), renderConfig: config, damage: .full)

        cache.update(
            frame: frame("aaa\nxyz\nccc"),
            renderConfig: config,
            damage: .partial([TerminalDirtySpan(row: 1, leftCol: 0, rightCol: 2)])
        )

        XCTAssertFalse(cache.stats.wasFullRebuild)
        XCTAssertEqual(cache.stats.rebuiltRowCount, 1)
    }

    /// A partial rebuild (reused rows + the one rebuilt row) must produce exactly
    /// the same flattened plan as a full rebuild of the new frame.
    func testPartialRebuildMatchesFullRebuild() {
        let changed = frame("aaa\nxyz\nccc")

        let partialCache = TerminalRenderPlanCache()
        partialCache.update(frame: frame("aaa\nbbb\nccc"), renderConfig: config, damage: .full)
        partialCache.update(
            frame: changed,
            renderConfig: config,
            damage: .partial([TerminalDirtySpan(row: 1, leftCol: 0, rightCol: 2)])
        )

        let fullCache = TerminalRenderPlanCache()
        fullCache.update(frame: changed, renderConfig: config, damage: .full)

        XCTAssertEqual(partialCache.plan, fullCache.plan)
    }

    func testConfigChangeForcesFullRebuildDespitePartialDamage() {
        let cache = TerminalRenderPlanCache()
        cache.update(frame: frame("aaa\nbbb\nccc"), renderConfig: config, damage: .full)

        var biggerFont = config
        biggerFont.fontSize = config.fontSize + 4

        cache.update(
            frame: frame("aaa\nbbb\nccc"),
            renderConfig: biggerFont,
            damage: .partial([TerminalDirtySpan(row: 0, leftCol: 0, rightCol: 2)])
        )

        XCTAssertTrue(cache.stats.wasFullRebuild)
        XCTAssertEqual(cache.stats.rebuiltRowCount, 3)
    }

    func testDimensionChangeForcesFullRebuild() {
        let cache = TerminalRenderPlanCache()
        cache.update(frame: frame("aaa\nbbb\nccc", cols: 3, rows: 3), renderConfig: config, damage: .full)

        cache.update(
            frame: frame("aaaa\nbbbb", cols: 4, rows: 2),
            renderConfig: config,
            damage: .partial([TerminalDirtySpan(row: 0, leftCol: 0, rightCol: 3)])
        )

        XCTAssertTrue(cache.stats.wasFullRebuild)
        XCTAssertEqual(cache.stats.totalRowCount, 2)
    }
}
