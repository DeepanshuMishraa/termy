import XCTest
@testable import TermySwift

final class TmuxLayoutTests: XCTestCase {
    func testSinglePane() {
        XCTAssertEqual(
            TmuxLayout.parse("bash,80x24,0,0,0"),
            .pane(id: 0, width: 80, height: 24, x: 0, y: 0)
        )
    }

    func testHorizontalSplit() {
        XCTAssertEqual(
            TmuxLayout.parse("a1b2,80x24,0,0{40x24,0,0,1,39x24,41,0,2}"),
            .horizontal([
                .pane(id: 1, width: 40, height: 24, x: 0, y: 0),
                .pane(id: 2, width: 39, height: 24, x: 41, y: 0),
            ])
        )
    }

    func testVerticalSplit() {
        XCTAssertEqual(
            TmuxLayout.parse("c3d4,80x24,0,0[80x12,0,0,1,80x11,0,13,2]"),
            .vertical([
                .pane(id: 1, width: 80, height: 12, x: 0, y: 0),
                .pane(id: 2, width: 80, height: 11, x: 0, y: 13),
            ])
        )
    }

    func testNestedSplit() {
        XCTAssertEqual(
            TmuxLayout.parse("e5f6,80x24,0,0{40x24,0,0,1,39x24,41,0[39x12,41,0,2,39x11,41,13,3]}"),
            .horizontal([
                .pane(id: 1, width: 40, height: 24, x: 0, y: 0),
                .vertical([
                    .pane(id: 2, width: 39, height: 12, x: 41, y: 0),
                    .pane(id: 3, width: 39, height: 11, x: 41, y: 13),
                ]),
            ])
        )
    }

    func testMalformedReturnsNil() {
        XCTAssertNil(TmuxLayout.parse("not-a-layout"))
        XCTAssertNil(TmuxLayout.parse(""))
    }
}
