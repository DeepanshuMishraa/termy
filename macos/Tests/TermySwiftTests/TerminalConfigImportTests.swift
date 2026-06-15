import XCTest
@testable import TermySwift

final class TerminalConfigImportTests: XCTestCase {
    func testNormalizeHexAcceptsCommonFormats() {
        XCTAssertEqual(TerminalConfigImport.normalizeHex("#1D2021"), "#1d2021")
        XCTAssertEqual(TerminalConfigImport.normalizeHex("0x1d2021"), "#1d2021")
        XCTAssertEqual(TerminalConfigImport.normalizeHex("\"#1d2021\""), "#1d2021")
        XCTAssertEqual(TerminalConfigImport.normalizeHex("1d2021"), "#1d2021")
        XCTAssertNil(TerminalConfigImport.normalizeHex("#12345"))
        XCTAssertNil(TerminalConfigImport.normalizeHex("nothex0"))
    }

    func testAlacrittyParsesFontAndSectionedColors() {
        let config = """
        [font.normal]
        family = "JetBrains Mono"

        [font]
        size = 13.5

        [colors.primary]
        background = "#1d2021"
        foreground = "#ebdbb2"

        [colors.cursor]
        text = "#1d2021"
        cursor = "#fabd2f"

        [colors.normal]
        black = "0x282828"
        red = "#cc241d"

        [colors.bright]
        black = "#928374"
        """
        let result = TerminalConfigImport.parse(config, format: "alacritty")
        XCTAssertEqual(result.fontFamily, "JetBrains Mono")
        XCTAssertEqual(result.fontSize, 13.5)
        XCTAssertEqual(result.colors["background"], "#1d2021")
        XCTAssertEqual(result.colors["foreground"], "#ebdbb2")
        XCTAssertEqual(result.colors["cursor"], "#fabd2f")
        XCTAssertEqual(result.colors["black"], "#282828")
        XCTAssertEqual(result.colors["red"], "#cc241d")
        XCTAssertEqual(result.colors["bright_black"], "#928374")
    }

    func testKittyParsesFontAndIndexedColors() {
        let config = """
        font_family Fira Code
        font_size 14.0
        foreground #ebdbb2
        background #1d2021
        cursor #fabd2f
        color0 #282828
        color9 #fb4934
        # a comment line
        """
        let result = TerminalConfigImport.parse(config, format: "kitty")
        XCTAssertEqual(result.fontFamily, "Fira Code")
        XCTAssertEqual(result.fontSize, 14.0)
        XCTAssertEqual(result.colors["foreground"], "#ebdbb2")
        XCTAssertEqual(result.colors["background"], "#1d2021")
        XCTAssertEqual(result.colors["cursor"], "#fabd2f")
        XCTAssertEqual(result.colors["black"], "#282828")
        XCTAssertEqual(result.colors["bright_red"], "#fb4934")
    }

    func testGhosttyParsesFontAndPalette() {
        let config = """
        font-family = "JetBrains Mono"
        font-size = 12
        foreground = #ebdbb2
        background = 1d2021
        cursor-color = #fabd2f
        palette = 0=#282828
        palette = 15=#fbf1c7
        """
        let result = TerminalConfigImport.parse(config, format: "ghostty")
        XCTAssertEqual(result.fontFamily, "JetBrains Mono")
        XCTAssertEqual(result.fontSize, 12)
        XCTAssertEqual(result.colors["foreground"], "#ebdbb2")
        XCTAssertEqual(result.colors["background"], "#1d2021")
        XCTAssertEqual(result.colors["cursor"], "#fabd2f")
        XCTAssertEqual(result.colors["black"], "#282828")
        XCTAssertEqual(result.colors["bright_white"], "#fbf1c7")
    }

    func testEmptyConfigImportsNothing() {
        XCTAssertTrue(TerminalConfigImport.parse("", format: "kitty").isEmpty)
        XCTAssertTrue(TerminalConfigImport.parse("unrelated = 1", format: "unknown").isEmpty)
    }

    func testITerm2ParsesDefaultProfileColors() {
        func color(_ r: Double, _ g: Double, _ b: Double) -> String {
            """
            <dict>
                <key>Red Component</key><real>\(r)</real>
                <key>Green Component</key><real>\(g)</real>
                <key>Blue Component</key><real>\(b)</real>
            </dict>
            """
        }
        let plist = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
        "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
            <key>Default Bookmark Guid</key><string>B</string>
            <key>New Bookmarks</key>
            <array>
                <dict>
                    <key>Guid</key><string>A</string>
                    <key>Background Color</key>\(color(1, 1, 1))
                </dict>
                <dict>
                    <key>Guid</key><string>B</string>
                    <key>Foreground Color</key>\(color(0.921568, 0.858823, 0.698039))
                    <key>Background Color</key>\(color(0.113725, 0.125490, 0.129411))
                    <key>Cursor Color</key>\(color(0.980392, 0.741176, 0.184313))
                    <key>Ansi 0 Color</key>\(color(0.156862, 0.156862, 0.156862))
                    <key>Ansi 15 Color</key>\(color(0.984313, 0.945098, 0.780392))
                </dict>
            </array>
        </dict>
        </plist>
        """
        let result = TerminalConfigImport.parseITerm2Data(Data(plist.utf8))
        // Default profile is "B", not the first bookmark "A".
        XCTAssertEqual(result.colors["background"], "#1d2021")
        XCTAssertEqual(result.colors["foreground"], "#ebdbb2")
        XCTAssertEqual(result.colors["cursor"], "#fabd2f")
        XCTAssertEqual(result.colors["black"], "#282828")
        XCTAssertEqual(result.colors["bright_white"], "#fbf1c7")
    }

    func testITerm2WithoutBookmarksIsEmpty() {
        let plist = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
        "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
            <key>New Bookmarks</key>
            <array/>
        </dict>
        </plist>
        """
        XCTAssertTrue(TerminalConfigImport.parseITerm2Data(Data(plist.utf8)).isEmpty)
    }

    func testNormalizeTriggerConvertsModifiers() {
        XCTAssertEqual(TerminalConfigImport.normalizeTrigger("ctrl+shift+c"), "ctrl-shift-c")
        XCTAssertEqual(TerminalConfigImport.normalizeTrigger("super+t"), "cmd-t")
        XCTAssertEqual(TerminalConfigImport.normalizeTrigger("option+left"), "alt-left")
        XCTAssertNil(TerminalConfigImport.normalizeTrigger(""))
    }

    func testKittyKeybindsMapHighConfidenceActionsOnly() {
        let config = """
        map ctrl+shift+c copy_to_clipboard
        map ctrl+shift+v paste_from_clipboard
        map cmd+t new_tab
        map ctrl+shift+e some_unmapped_action
        """
        let result = TerminalConfigImport.parse(config, format: "kitty")
        XCTAssertEqual(result.keybinds, [
            ImportedKeybind(trigger: "ctrl-shift-c", action: "copy"),
            ImportedKeybind(trigger: "ctrl-shift-v", action: "paste"),
            ImportedKeybind(trigger: "cmd-t", action: "new_tab"),
        ])
    }

    func testGhosttyKeybindsMapSplitsAndClose() {
        let config = """
        keybind = ctrl+shift+c=copy_to_clipboard
        keybind = cmd+d=new_split:right
        keybind = cmd+shift+d=new_split:down
        keybind = cmd+w=close_surface
        keybind = cmd+x=unmapped
        """
        let result = TerminalConfigImport.parse(config, format: "ghostty")
        XCTAssertEqual(result.keybinds, [
            ImportedKeybind(trigger: "ctrl-shift-c", action: "copy"),
            ImportedKeybind(trigger: "cmd-d", action: "split_pane_vertical"),
            ImportedKeybind(trigger: "cmd-shift-d", action: "split_pane_horizontal"),
            ImportedKeybind(trigger: "cmd-w", action: "close_pane_or_tab"),
        ])
    }
}
