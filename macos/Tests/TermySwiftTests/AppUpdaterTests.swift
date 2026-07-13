import Foundation
import XCTest
@testable import TermySwift

@MainActor
final class AppUpdaterTests: XCTestCase {
    func testOfflineFetchPropagatesNetworkFailureWithoutFabricatingARelease() async {
        let updater = AppUpdater { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/vnd.github+json")
            throw URLError(.notConnectedToInternet)
        }

        do {
            _ = try await updater.fetchLatest()
            XCTFail("offline update fetch unexpectedly succeeded")
        } catch let error as URLError {
            XCTAssertEqual(error.code, .notConnectedToInternet)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testStableReleaseOutranksPrereleaseButLaterPrereleaseCanAdvance() {
        XCTAssertFalse(AppUpdater.isNewer("1.2.0-rc.1", than: "1.2.0"))
        XCTAssertTrue(AppUpdater.isNewer("1.2.0-rc.2", than: "1.2.0-rc.1"))
        XCTAssertTrue(AppUpdater.isNewer("1.2.0", than: "1.2.0-rc.9"))
        XCTAssertTrue(AppUpdater.isNewer("1.3", than: "1.2.99"))
    }

    func testMalformedVersionsNeverTriggerAnUpdate() {
        XCTAssertFalse(AppUpdater.isNewer("latest", than: "1.0.0"))
        XCTAssertFalse(AppUpdater.isNewer("1..2", than: "1.0.0"))
        XCTAssertFalse(AppUpdater.isNewer("1.2.0", than: "unknown"))
    }

    func testGitHubReleaseResponseRequiresSuccessValidVersionAndTrustedDownloadURL() throws {
        let url = try XCTUnwrap(URL(string: "https://api.github.com/repos/lassejlv/termy/releases/latest"))
        let success = try XCTUnwrap(HTTPURLResponse(
            url: url,
            statusCode: 200,
            httpVersion: nil,
            headerFields: nil
        ))
        let release = try AppUpdater.release(
            from: Data(#"{"tag_name":"v1.2.3","html_url":"https://github.com/lassejlv/termy/releases/tag/v1.2.3"}"#.utf8),
            response: success
        )
        XCTAssertEqual(release.version, "1.2.3")
        XCTAssertEqual(release.url.host, "github.com")

        let failure = try XCTUnwrap(HTTPURLResponse(
            url: url,
            statusCode: 503,
            httpVersion: nil,
            headerFields: nil
        ))
        XCTAssertThrowsError(try AppUpdater.release(from: Data(), response: failure)) { error in
            XCTAssertEqual(error as? AppUpdater.UpdateError, .invalidResponse)
        }
        XCTAssertThrowsError(try AppUpdater.release(
            from: Data(#"{"tag_name":"nightly","html_url":"https://github.com/lassejlv/termy/releases"}"#.utf8),
            response: success
        )) { error in
            XCTAssertEqual(error as? AppUpdater.UpdateError, .invalidVersion)
        }
        XCTAssertThrowsError(try AppUpdater.release(
            from: Data(#"{"tag_name":"1.2.3","html_url":"https://example.com/fake"}"#.utf8),
            response: success
        )) { error in
            XCTAssertEqual(error as? AppUpdater.UpdateError, .invalidResponse)
        }
    }
}
