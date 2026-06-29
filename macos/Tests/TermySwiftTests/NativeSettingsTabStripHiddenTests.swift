import XCTest
@testable import TermySwift

/// The native macOS host uses the native macOS tab bar, so the shared
/// tab-strip settings (still consumed by the GPUI desktop app) should be
/// hidden from the macOS Settings UI while remaining present in the raw
/// schema that's parity-tested against Rust.
final class NativeSettingsTabStripHiddenTests: XCTestCase {
    func testTabStripKeysAreHiddenFromNativeSettingsSections() throws {
        let schema = try SettingsBridge.loadSchema(contents: "")
        let visibleKeys = Set(
            schema.nativeSettingsSections
                .flatMap { $0.groups ?? [] }
                .flatMap(\.settings)
                .map(\.key)
        )

        for key in SettingsSectionModel.hiddenNativeTabStripKeys {
            XCTAssertFalse(
                visibleKeys.contains(key),
                "\(key) should be hidden from the macOS Settings UI"
            )
        }
    }

    func testTabStripKeysRemainInRawSchemaForParity() throws {
        let schema = try SettingsBridge.loadSchema(contents: "")
        let rawKeys = Set(
            schema.sections
                .flatMap { $0.groups ?? [] }
                .flatMap(\.settings)
                .map(\.key)
        )

        for key in SettingsSectionModel.hiddenNativeTabStripKeys {
            XCTAssertTrue(
                rawKeys.contains(key),
                "\(key) must remain in the shared schema (GPUI desktop app still uses it)"
            )
        }
    }
}
