import Foundation
import XCTest
@testable import TermySwift

final class NativeSoakReportTests: XCTestCase {
    func testReportRoundTripsForCIValidation() throws {
        let report = NativeSoakReport(
            durationSeconds: 7_200,
            cycles: 1_000,
            outputLines: 200_000,
            tabsOpened: 125,
            initialRSSMiB: 120,
            maximumRSSMiB: 180,
            finalRSSMiB: 140,
            rssGrowthMiB: 20,
            initialWindowCount: 1,
            maximumWindowCount: 2,
            finalWindowCount: 1,
            maximumPaneCount: 2,
            finalPaneCount: 1,
            errors: []
        )

        let data = try JSONEncoder().encode(report)

        XCTAssertEqual(try JSONDecoder().decode(NativeSoakReport.self, from: data), report)
    }
}
