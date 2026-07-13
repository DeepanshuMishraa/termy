import Foundation
import XCTest
@testable import TermySwift

final class TerminalWorkspacePersistenceFailureTests: XCTestCase {
    func testMissingStateLoadsAsEmpty() throws {
        let fixture = makeFixture()
        defer { fixture.cleanup() }

        XCTAssertEqual(
            try fixture.persistence.loadState(),
            TerminalWorkspacePersistenceState()
        )
    }

    func testTruncatedStateSurfacesDecodeFailureAndResetRecovers() throws {
        let fixture = makeFixture()
        defer { fixture.cleanup() }
        try FileManager.default.createDirectory(
            at: fixture.fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data(#"{"version":1,"last_session":"#.utf8).write(to: fixture.fileURL)

        XCTAssertThrowsError(try fixture.persistence.loadState()) { error in
            XCTAssertTrue(error is DecodingError)
        }

        try fixture.persistence.reset()
        XCTAssertEqual(try fixture.persistence.loadState(), TerminalWorkspacePersistenceState())
    }

    func testIncompatibleStateVersionIsRejected() throws {
        let fixture = makeFixture()
        defer { fixture.cleanup() }
        try FileManager.default.createDirectory(
            at: fixture.fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data(#"{"version":99,"last_session":null,"layouts":[]}"#.utf8).write(to: fixture.fileURL)

        XCTAssertThrowsError(try fixture.persistence.loadState()) { error in
            XCTAssertEqual(error as? TerminalWorkspacePersistenceError, .unsupportedVersion(99))
        }
    }

    func testFailedParentDirectoryCreationSurfacesWriteError() throws {
        let fixture = makeFixture()
        defer { fixture.cleanup() }
        let parent = fixture.fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parent.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("not a directory".utf8).write(to: parent)
        let state = TerminalWorkspacePersistenceState(
            lastSession: TerminalWorkspaceSnapshot(tabs: []),
            layouts: []
        )

        XCTAssertThrowsError(try fixture.persistence.saveState(state))
    }

    private func makeFixture() -> (
        persistence: TerminalWorkspacePersistence,
        fileURL: URL,
        cleanup: () -> Void
    ) {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("TermyPersistenceTests-\(UUID().uuidString)", isDirectory: true)
        let fileURL = root
            .appendingPathComponent("state", isDirectory: true)
            .appendingPathComponent("native-workspace.json")
        return (
            TerminalWorkspacePersistence(fileURL: fileURL),
            fileURL,
            { try? FileManager.default.removeItem(at: root) }
        )
    }
}
