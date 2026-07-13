import Foundation
import XCTest
@testable import TermySwift

final class TermyDiagnosticsTests: XCTestCase {
    func testReportIncludesMetadataDiagnosticsAndLogsWithoutSensitiveInputs() {
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let snapshot = TermyDiagnosticsSnapshot(
            generatedAt: date,
            appVersion: "1.2.3",
            buildVersion: "42",
            bundleIdentifier: "com.lassevestergaard.termy",
            architecture: "arm64",
            operatingSystem: "macOS 26.0",
            configPath: "/Users/tester/.config/termy/config",
            configDiagnostics: [
                TermyConfigDiagnostic(
                    lineNumber: 7,
                    kind: .invalidValue,
                    message: "api_key = super-secret-value"
                )
            ],
            recentLogs: [
                TermyDiagnosticsLogEntry(
                    date: date,
                    category: "lifecycle",
                    level: "error",
                    message: "failed password=hunter2 at /Users/tester/project"
                )
            ],
            logCollectionError: nil
        )

        let report = TermyDiagnosticsReport.render(snapshot, homeDirectory: "/Users/tester")

        XCTAssertTrue(report.contains("Version: 1.2.3"))
        XCTAssertTrue(report.contains("Architecture: arm64"))
        XCTAssertTrue(report.contains("Config: ~/.config/termy/config"))
        XCTAssertTrue(report.contains("[invalid-value] line 7"))
        XCTAssertTrue(report.contains("[error] [lifecycle]"))
        XCTAssertTrue(report.contains("<redacted>"))
        XCTAssertFalse(report.contains("super-secret-value"))
        XCTAssertFalse(report.contains("hunter2"))
        XCTAssertFalse(report.contains("/Users/tester"))
        XCTAssertTrue(report.contains("Terminal output, configuration contents, and environment variables are intentionally excluded."))
    }

    func testSanitizerRedactsCredentialsAndTokens() {
        let text = "Authorization: Bearer-abc token=topsecret https://user:pass@example.com ghp-abcdefghijk"
        let sanitized = TermyDiagnosticsReport.sanitize(text, homeDirectory: "/Users/tester")

        XCTAssertFalse(sanitized.contains("Bearer-abc"))
        XCTAssertFalse(sanitized.contains("topsecret"))
        XCTAssertFalse(sanitized.contains("user:pass"))
        XCTAssertFalse(sanitized.contains("ghp-abcdefghijk"))
        XCTAssertGreaterThanOrEqual(sanitized.components(separatedBy: "<redacted>").count, 4)
    }
}
