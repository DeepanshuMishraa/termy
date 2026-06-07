import XCTest
@testable import TermySwift

final class TerminalBlockGlyphsTests: XCTestCase {
    private func rects(_ character: Character) -> [TerminalBlockRect]? {
        TerminalBlockGlyphs.geometry(for: character)
    }

    // Geometry parity with `crates/terminal_ui/src/grid.rs::block_element_geometry`.

    func testHalvesAndFullBlock() {
        XCTAssertEqual(rects("\u{2580}"), [block(0, 0, 1, 0.5)]) // ▀
        XCTAssertEqual(rects("\u{2584}"), [block(0, 0.5, 1, 1)]) // ▄
        XCTAssertEqual(rects("\u{2588}"), [block(0, 0, 1, 1)]) // █
        XCTAssertEqual(rects("\u{258C}"), [block(0, 0, 0.5, 1)]) // ▌
        XCTAssertEqual(rects("\u{2590}"), [block(0.5, 0, 1, 1)]) // ▐
    }

    func testEighthBlocks() {
        XCTAssertEqual(rects("\u{2581}"), [block(0, 7.0 / 8.0, 1, 1)]) // ▁
        XCTAssertEqual(rects("\u{2587}"), [block(0, 1.0 / 8.0, 1, 1)]) // ▇
        XCTAssertEqual(rects("\u{2589}"), [block(0, 0, 7.0 / 8.0, 1)]) // ▉
        XCTAssertEqual(rects("\u{258F}"), [block(0, 0, 1.0 / 8.0, 1)]) // ▏
        XCTAssertEqual(rects("\u{2594}"), [block(0, 0, 1, 1.0 / 8.0)]) // ▔
        XCTAssertEqual(rects("\u{2595}"), [block(7.0 / 8.0, 0, 1, 1)]) // ▕
    }

    func testShadesFillCellWithPartialAlpha() {
        XCTAssertEqual(rects("\u{2591}"), [block(0, 0, 1, 1, alpha: 0.25)]) // ░
        XCTAssertEqual(rects("\u{2592}"), [block(0, 0, 1, 1, alpha: 0.50)]) // ▒
        XCTAssertEqual(rects("\u{2593}"), [block(0, 0, 1, 1, alpha: 0.75)]) // ▓
    }

    func testQuadrants() {
        XCTAssertEqual(rects("\u{2598}"), [block(0, 0, 0.5, 0.5)]) // ▘
        XCTAssertEqual(rects("\u{259D}"), [block(0.5, 0, 1, 0.5)]) // ▝
        XCTAssertEqual(rects("\u{2596}"), [block(0, 0.5, 0.5, 1)]) // ▖
        XCTAssertEqual(rects("\u{2597}"), [block(0.5, 0.5, 1, 1)]) // ▗
        XCTAssertEqual(
            rects("\u{2599}"), // ▙
            [block(0, 0, 0.5, 0.5), block(0, 0.5, 0.5, 1), block(0.5, 0.5, 1, 1)]
        )
        XCTAssertEqual(
            rects("\u{259E}"), // ▞
            [block(0.5, 0, 1, 0.5), block(0, 0.5, 0.5, 1)]
        )
    }

    func testEveryBlockElementCodepointHasGeometry() {
        for codepoint in 0x2580...0x259F {
            let scalar = Unicode.Scalar(codepoint)!
            XCTAssertNotNil(
                rects(Character(scalar)),
                "U+\(String(codepoint, radix: 16, uppercase: true)) should have geometry"
            )
        }
    }

    func testNonBlockCharactersReturnNil() {
        XCTAssertNil(rects("a"))
        XCTAssertNil(rects(" "))
        XCTAssertNil(rects("\u{2500}")) // ─ box drawing is out of scope here
        XCTAssertNil(rects("\u{25A0}")) // ■ geometric shapes
    }

    // Seam-freeness: snapping must make adjacent full blocks share an identical
    // boundary for ANY fractional cell size, padding, and scale — exercised
    // through the real snappedRect used by the renderer. cellHeight 14.7 at 1x
    // scale is a known seam case for naive per-cell-origin snapping.

    func testAdjacentRowsShareSnappedBoundary() {
        let fullCell = block(0, 0, 1, 1)
        for cellHeight in [19.6, 14.7, 21.333, 11.1, 28.8] {
            for paddingY in [8.0, 7.3, 0.0, 3.14159] {
                for scale in [1.0, 2.0, 3.0] {
                    for row in 0..<50 {
                        let cell = snapped(fullCell, row: row, cellHeight: cellHeight, paddingY: paddingY, scale: scale)
                        let next = snapped(fullCell, row: row + 1, cellHeight: cellHeight, paddingY: paddingY, scale: scale)
                        XCTAssertEqual(
                            cell?.maxY,
                            next?.minY,
                            "row \(row) bottom must equal row \(row + 1) top "
                                + "(cellHeight \(cellHeight), padding \(paddingY), scale \(scale))"
                        )
                    }
                }
            }
        }
    }

    func testAdjacentColumnsShareSnappedBoundary() {
        let fullCell = block(0, 0, 1, 1)
        for cellWidth in [8.4, 7.7, 9.0, 6.333] {
            for scale in [1.0, 2.0, 3.0] {
                for col in 0..<200 {
                    let cell = snapped(fullCell, col: col, cellWidth: cellWidth, scale: scale)
                    let next = snapped(fullCell, col: col + 1, cellWidth: cellWidth, scale: scale)
                    XCTAssertEqual(
                        cell?.maxX,
                        next?.minX,
                        "col \(col) right must equal col \(col + 1) left "
                            + "(cellWidth \(cellWidth), scale \(scale))"
                    )
                }
            }
        }
    }

    func testHalfBlocksMeetInsideTheCell() {
        // ▀ over ▄ in the same cell: the upper half's bottom must equal the
        // lower half's top because both edges use the same 0.5 fraction.
        for cellHeight in [19.6, 14.7, 11.1] {
            for scale in [1.0, 2.0] {
                let upper = snapped(block(0, 0, 1, 0.5), row: 3, cellHeight: cellHeight, scale: scale)
                let lower = snapped(block(0, 0.5, 1, 1), row: 3, cellHeight: cellHeight, scale: scale)
                XCTAssertEqual(upper?.maxY, lower?.minY)
            }
        }
    }

    // Render plan classification: block elements become block glyphs, not text.

    func testRenderPlanClassifiesBlockElementsAsBlockGlyphs() {
        let cache = TerminalRenderPlanCache()
        cache.update(
            frame: TerminalFrame.plainTextPreview("a\u{2588}b", cols: 3, rows: 1),
            renderConfig: .default,
            damage: .full
        )

        XCTAssertEqual(cache.plan.blockGlyphs.count, 1)
        XCTAssertEqual(cache.plan.blockGlyphs.first?.col, 1)
        XCTAssertEqual(cache.plan.blockGlyphs.first?.rects, [block(0, 0, 1, 1)])
        XCTAssertEqual(cache.plan.textSegments.map(\.text), ["a", "b"])
    }

    func testRenderPlanKeepsPlainTextOutOfBlockGlyphs() {
        let cache = TerminalRenderPlanCache()
        cache.update(
            frame: TerminalFrame.plainTextPreview("abc", cols: 3, rows: 1),
            renderConfig: .default,
            damage: .full
        )

        XCTAssertTrue(cache.plan.blockGlyphs.isEmpty)
        XCTAssertEqual(cache.plan.textSegments.map(\.text), ["abc"])
    }

    private func block(
        _ left: Double,
        _ top: Double,
        _ right: Double,
        _ bottom: Double,
        alpha: Double = 1.0
    ) -> TerminalBlockRect {
        TerminalBlockRect(left: left, top: top, right: right, bottom: bottom, alpha: alpha)
    }

    private func snapped(
        _ rect: TerminalBlockRect,
        col: Int = 0,
        row: Int = 0,
        cellWidth: Double = 8.4,
        cellHeight: Double = 19.6,
        paddingX: Double = 12.0,
        paddingY: Double = 8.0,
        scale: Double = 2.0
    ) -> CGRect? {
        TerminalBlockGlyphs.snappedRect(
            rect,
            col: col,
            row: row,
            cellWidth: cellWidth,
            cellHeight: cellHeight,
            paddingX: paddingX,
            paddingY: paddingY,
            scale: scale
        )
    }
}
