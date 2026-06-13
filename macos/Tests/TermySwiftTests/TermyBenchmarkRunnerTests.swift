import XCTest
@testable import TermySwift

final class TermyBenchmarkRunnerTests: XCTestCase {
    func testBenchmarkWorkloadProducesMetrics() throws {
        let result = TermyBenchmarkRunner.run(cols: 40, rows: 10, frames: 40)

        XCTAssertGreaterThanOrEqual(result.metrics.frameUpdates, 1)
        XCTAssertGreaterThanOrEqual(result.metrics.fullFrameUpdates, 1, "the forced full update seeds the workload")
        XCTAssertGreaterThanOrEqual(result.metrics.presentedFrames, 1)

        let json = result.metrics.encodedJSON
        XCTAssertTrue(json.contains("\"frameUpdates\":"))
        XCTAssertNotEqual(json, "{}")
    }

    func testPercentilesAreOrderedAndBounded() {
        let times = TermyBenchmarkRunner.percentiles(fromMicros: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100])
        XCTAssertEqual(times.samples, 10)
        XCTAssertEqual(times.meanMicros, 55, accuracy: 0.001)
        XCTAssertLessThanOrEqual(times.p50Micros, times.p95Micros)
        XCTAssertLessThanOrEqual(times.p95Micros, times.p99Micros)
        XCTAssertLessThanOrEqual(times.p99Micros, 100)
    }

    func testPercentilesEmptyIsZero() {
        let times = TermyBenchmarkRunner.percentiles(fromMicros: [])
        XCTAssertEqual(times, BenchmarkBuildTimes())
        XCTAssertEqual(times.encodedJSON.isEmpty, false)
    }
}
