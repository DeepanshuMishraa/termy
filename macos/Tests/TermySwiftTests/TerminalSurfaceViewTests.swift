import XCTest
@testable import TermySwift

final class TerminalSurfaceViewTests: XCTestCase {
    func testInactivePaneContentOpacityFollowsFocusEffect() {
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: true,
                effect: .softSpotlight,
                strength: 0.6
            ),
            1.0
        )
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: false,
                isFocused: false,
                effect: .softSpotlight,
                strength: 0.6
            ),
            1.0
        )
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: false,
                effect: .off,
                strength: 0.6
            ),
            1.0
        )

        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: false,
                effect: .softSpotlight,
                strength: 0.6
            ),
            0.796,
            accuracy: 0.001
        )
        XCTAssertEqual(
            TerminalSurfaceView.paneContentOpacity(
                showsFocusBorder: true,
                isFocused: false,
                effect: .cinematic,
                strength: 2.0
            ),
            0.62
        )
    }
}
