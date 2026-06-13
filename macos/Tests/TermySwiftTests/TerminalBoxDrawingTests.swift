import XCTest
@testable import TermySwift

final class TerminalBoxDrawingTests: XCTestCase {
    // Reference dimensions: 14pt font → light stroke ceil(14 * 0.0675) = 1pt,
    // heavy = 2pt, in an 8 x 20 cell.
    private let cellWidth = 8.0
    private let cellHeight = 20.0
    private let fontSize = 14.0

    private func geometry(_ character: Character) -> [TerminalBlockRect]? {
        TerminalBoxDrawing.rectGeometry(
            for: character,
            cellWidth: cellWidth,
            cellHeight: cellHeight,
            fontSize: fontSize
        )
    }

    func testStrokeWidthScalesWithFontSize() {
        XCTAssertEqual(TerminalBoxDrawing.strokeWidth(fontSize: 14), 1)
        XCTAssertEqual(TerminalBoxDrawing.strokeWidth(fontSize: 15), 2) // ceil(1.0125)
        XCTAssertEqual(TerminalBoxDrawing.strokeWidth(fontSize: 30), 3) // ceil(2.025)
        XCTAssertEqual(TerminalBoxDrawing.strokeWidth(fontSize: 4), 1) // floor of 1
    }

    // Segment table parity with grid.rs::box_draw_segments.

    func testSegmentsForCommonConnectors() {
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{2500}"), // ─
            TerminalBoxSegments(up: .none, down: .none, left: .light, right: .light)
        )
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{2502}"), // │
            TerminalBoxSegments(up: .light, down: .light, left: .none, right: .none)
        )
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{250C}"), // ┌
            TerminalBoxSegments(up: .none, down: .light, left: .none, right: .light)
        )
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{253C}"), // ┼
            TerminalBoxSegments(up: .light, down: .light, left: .light, right: .light)
        )
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{2550}"), // ═
            TerminalBoxSegments(up: .none, down: .none, left: .double, right: .double)
        )
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{2554}"), // ╔
            TerminalBoxSegments(up: .none, down: .double, left: .none, right: .double)
        )
    }

    func testDashedVariantsMapToSolid() {
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{2504}"), // ┄
            TerminalBoxDrawing.segments(for: "\u{2500}")
        )
        XCTAssertEqual(
            TerminalBoxDrawing.segments(for: "\u{250B}"), // ┋
            TerminalBoxDrawing.segments(for: "\u{2503}")
        )
    }

    func testEveryRectangularConnectorHasGeometry() {
        for codepoint in 0x2500...0x257F {
            let character = Character(Unicode.Scalar(codepoint)!)
            let hex = String(codepoint, radix: 16, uppercase: true)
            if (0x256D...0x2573).contains(codepoint) {
                XCTAssertNil(
                    TerminalBoxDrawing.segments(for: character),
                    "U+\(hex) is a rounded corner/diagonal, not a rect connector"
                )
                XCTAssertNotNil(
                    TerminalBoxDrawing.strokeKind(for: character),
                    "U+\(hex) should be a stroke glyph"
                )
            } else {
                XCTAssertNotNil(
                    TerminalBoxDrawing.segments(for: character),
                    "U+\(hex) should have segments"
                )
                let rects = geometry(character)
                XCTAssertNotNil(rects, "U+\(hex) should produce geometry")
                XCTAssertFalse(rects!.isEmpty, "U+\(hex) should produce at least one rect")
            }
        }
    }

    func testStrokeKindClassification() {
        XCTAssertEqual(TerminalBoxDrawing.strokeKind(for: "\u{256D}"), .roundedCorner) // ╭
        XCTAssertEqual(TerminalBoxDrawing.strokeKind(for: "\u{2570}"), .roundedCorner) // ╰
        XCTAssertEqual(TerminalBoxDrawing.strokeKind(for: "\u{2571}"), .diagonal) // ╱
        XCTAssertEqual(TerminalBoxDrawing.strokeKind(for: "\u{2573}"), .diagonal) // ╳
        XCTAssertNil(TerminalBoxDrawing.strokeKind(for: "\u{2500}"))
        XCTAssertNil(TerminalBoxDrawing.strokeKind(for: "a"))
    }

    // Geometry shape checks. With light=1, cell 8x20: hLightTop=9.5,
    // hLightBottom=10.5, vLightLeft=3.5, vLightRight=4.5.

    func testHorizontalLightLineSpansFullWidthCenteredVertically() {
        let rects = geometry("\u{2500}")! // ─
        XCTAssertEqual(rects.count, 1)
        let rect = rects[0]
        XCTAssertEqual(rect.left, 0)
        XCTAssertEqual(rect.right, 1)
        XCTAssertEqual(rect.top, 9.5 / cellHeight, accuracy: 1e-9)
        XCTAssertEqual(rect.bottom, 10.5 / cellHeight, accuracy: 1e-9)
    }

    func testVerticalLightLineSpansFullHeightCenteredHorizontally() {
        let rects = geometry("\u{2502}")! // │
        XCTAssertEqual(rects.count, 1)
        let rect = rects[0]
        XCTAssertEqual(rect.top, 0)
        XCTAssertEqual(rect.bottom, 1)
        XCTAssertEqual(rect.left, 3.5 / cellWidth, accuracy: 1e-9)
        XCTAssertEqual(rect.right, 4.5 / cellWidth, accuracy: 1e-9)
    }

    func testLightCrossMergesIntoTwoRects() {
        // ┼ pushes four arm rects; collinear merging joins the two horizontal
        // and the two vertical arms back into one full-width and one
        // full-height rect.
        let rects = geometry("\u{253C}")!
        XCTAssertEqual(rects.count, 2)
        XCTAssertTrue(rects.contains { $0.left == 0 && $0.right == 1 })
        XCTAssertTrue(rects.contains { $0.top == 0 && $0.bottom == 1 })
    }

    func testDoubleHorizontalLineIsTwoParallelRects() {
        let rects = geometry("\u{2550}")! // ═
        XCTAssertEqual(rects.count, 2)
        for rect in rects {
            XCTAssertEqual(rect.left, 0)
            XCTAssertEqual(rect.right, 1)
        }
        XCTAssertNotEqual(rects[0].top, rects[1].top)
    }

    func testCornerArmsMeetWithoutGap() {
        // ┌ : the down arm and right arm must overlap at the corner so the
        // connector is contiguous.
        let rects = geometry("\u{250C}")!
        XCTAssertEqual(rects.count, 2)
        let down = rects.first { $0.bottom == 1 }!
        let right = rects.first { $0.right == 1 }!
        XCTAssertLessThanOrEqual(down.top, right.bottom)
        XCTAssertLessThanOrEqual(right.left, down.right)
    }

    // Render plan classification.

    func testRenderPlanClassifiesBoxDrawingAsBlockGlyphsAndStrokeGlyphs() {
        let cache = TerminalRenderPlanCache()
        cache.update(
            frame: TerminalFrame.plainTextPreview("\u{256D}\u{2500}x", cols: 3, rows: 1),
            renderConfig: .default,
            damage: .full
        )

        XCTAssertEqual(cache.plan.strokeGlyphs.count, 1)
        XCTAssertEqual(cache.plan.strokeGlyphs.first?.kind, .roundedCorner)
        XCTAssertEqual(cache.plan.strokeGlyphs.first?.col, 0)
        XCTAssertEqual(cache.plan.blockGlyphs.count, 1)
        XCTAssertEqual(cache.plan.blockGlyphs.first?.col, 1)
        XCTAssertEqual(cache.plan.textSegments.map(\.text), ["x"])
    }
}
