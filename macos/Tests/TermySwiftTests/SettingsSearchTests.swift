import XCTest
@testable import TermySwift

final class SettingsSearchTests: XCTestCase {
    private func setting(_ key: String, _ title: String, _ description: String = "") -> Setting {
        Setting(key: key, title: title, description: description, kind: .boolean, value: nil, choices: nil)
    }

    private var sampleSections: [SettingsSectionModel] {
        [
            SettingsSectionModel(
                id: "terminal",
                label: "Terminal",
                systemImage: "terminal",
                groups: [
                    SettingsGroup(label: "Scrolling", settings: [
                        setting("scrollback_history", "Scrollback History", "Lines retained in scrollback"),
                        setting("cursor_blink", "Cursor Blink", "Blink the cursor"),
                    ]),
                ],
                colors: nil,
                keybinds: nil
            ),
            SettingsSectionModel(
                id: "appearance",
                label: "Appearance",
                systemImage: "paintbrush",
                groups: [
                    SettingsGroup(label: "Font", settings: [
                        setting("font_size", "Font Size", "Terminal font size"),
                    ]),
                ],
                colors: nil,
                keybinds: nil
            ),
        ]
    }

    func testMatchesTitleDescriptionAndKey() {
        XCTAssertTrue(SettingsSearch.matches(setting("font_size", "Font Size", "size of text"), query: "font"))
        XCTAssertTrue(SettingsSearch.matches(setting("font_size", "Font Size", "size of text"), query: "TEXT"))
        XCTAssertTrue(SettingsSearch.matches(setting("font_size", "Font Size", "size of text"), query: "font_s"))
        XCTAssertFalse(SettingsSearch.matches(setting("font_size", "Font Size", "size of text"), query: "cursor"))
    }

    func testResultsFlattenAcrossSections() {
        let results = SettingsSearch.results(in: sampleSections, query: "scroll")
        XCTAssertEqual(results.map(\.setting.key), ["scrollback_history"])
        XCTAssertEqual(results.first?.sectionLabel, "Terminal")
    }

    func testEmptyQueryReturnsNoResults() {
        XCTAssertTrue(SettingsSearch.results(in: sampleSections, query: "   ").isEmpty)
    }

    func testMatchSpansSections() {
        let results = SettingsSearch.results(in: sampleSections, query: "size")
        XCTAssertEqual(Set(results.map(\.setting.key)), ["font_size"])
    }
}
