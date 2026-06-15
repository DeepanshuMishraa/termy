import Foundation

/// Settings imported from another terminal's config: font family/size plus the
/// color palette (foreground/background/cursor and the 16 ANSI colors).
struct ImportedSettings: Equatable {
    var fontFamily: String?
    var fontSize: Double?
    /// Canonical Termy color key (`foreground`, `background`, `cursor`,
    /// `black`…`bright_white`) → `#rrggbb`.
    var colors: [String: String] = [:]
    /// Keybinds confidently mapped to Termy actions (others are skipped).
    var keybinds: [ImportedKeybind] = []

    var rootValues: [String: String] {
        var values: [String: String] = [:]
        if let fontFamily, !fontFamily.isEmpty {
            values["font_family"] = fontFamily
        }
        if let fontSize, fontSize > 0 {
            values["font_size"] = String(format: "%g", fontSize)
        }
        return values
    }

    var isEmpty: Bool {
        rootValues.isEmpty && colors.isEmpty && keybinds.isEmpty
    }
}

struct ImportedKeybind: Equatable {
    var trigger: String
    var action: String
}

struct ImportableTerminal: Identifiable {
    let id: String
    let name: String
    let configPath: URL

    var isDetected: Bool {
        FileManager.default.fileExists(atPath: configPath.path)
    }
}

private struct ITerm2Preferences: Decodable {
    var defaultBookmarkGuid: String?
    var newBookmarks: [ITerm2Profile]

    enum CodingKeys: String, CodingKey {
        case defaultBookmarkGuid = "Default Bookmark Guid"
        case newBookmarks = "New Bookmarks"
    }
}

private struct ITerm2Profile: Decodable {
    var guid: String?
    var foreground: ITerm2Color?
    var background: ITerm2Color?
    var cursor: ITerm2Color?
    var ansi0: ITerm2Color?
    var ansi1: ITerm2Color?
    var ansi2: ITerm2Color?
    var ansi3: ITerm2Color?
    var ansi4: ITerm2Color?
    var ansi5: ITerm2Color?
    var ansi6: ITerm2Color?
    var ansi7: ITerm2Color?
    var ansi8: ITerm2Color?
    var ansi9: ITerm2Color?
    var ansi10: ITerm2Color?
    var ansi11: ITerm2Color?
    var ansi12: ITerm2Color?
    var ansi13: ITerm2Color?
    var ansi14: ITerm2Color?
    var ansi15: ITerm2Color?

    var ansiColors: [ITerm2Color?] {
        [
            ansi0, ansi1, ansi2, ansi3, ansi4, ansi5, ansi6, ansi7,
            ansi8, ansi9, ansi10, ansi11, ansi12, ansi13, ansi14, ansi15,
        ]
    }

    enum CodingKeys: String, CodingKey {
        case guid = "Guid"
        case foreground = "Foreground Color"
        case background = "Background Color"
        case cursor = "Cursor Color"
        case ansi0 = "Ansi 0 Color"
        case ansi1 = "Ansi 1 Color"
        case ansi2 = "Ansi 2 Color"
        case ansi3 = "Ansi 3 Color"
        case ansi4 = "Ansi 4 Color"
        case ansi5 = "Ansi 5 Color"
        case ansi6 = "Ansi 6 Color"
        case ansi7 = "Ansi 7 Color"
        case ansi8 = "Ansi 8 Color"
        case ansi9 = "Ansi 9 Color"
        case ansi10 = "Ansi 10 Color"
        case ansi11 = "Ansi 11 Color"
        case ansi12 = "Ansi 12 Color"
        case ansi13 = "Ansi 13 Color"
        case ansi14 = "Ansi 14 Color"
        case ansi15 = "Ansi 15 Color"
    }
}

private struct ITerm2Color: Decodable {
    var red: Double
    var green: Double
    var blue: Double

    enum CodingKeys: String, CodingKey {
        case red = "Red Component"
        case green = "Green Component"
        case blue = "Blue Component"
    }
}

/// Detects and imports settings from other terminal emulators' config files.
enum TerminalConfigImport {
    /// ANSI color names in palette order (index 0–15), used to map `color0`/
    /// `palette = 0=…` style entries to Termy's canonical color keys.
    static let ansiColorNames = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
        "bright_black", "bright_red", "bright_green", "bright_yellow",
        "bright_blue", "bright_magenta", "bright_cyan", "bright_white",
    ]

    private static let normalColorNames = Set(ansiColorNames.prefix(8))

    static func candidates() -> [ImportableTerminal] {
        let home = FileManager.default.homeDirectoryForCurrentUser
        func path(_ relative: String) -> URL {
            home.appendingPathComponent(relative)
        }
        return [
            ImportableTerminal(
                id: "alacritty",
                name: "Alacritty",
                configPath: path(".config/alacritty/alacritty.toml")
            ),
            ImportableTerminal(
                id: "kitty",
                name: "Kitty",
                configPath: path(".config/kitty/kitty.conf")
            ),
            ImportableTerminal(
                id: "ghostty",
                name: "Ghostty",
                configPath: path(".config/ghostty/config")
            ),
            ImportableTerminal(
                id: "iterm2",
                name: "iTerm2",
                configPath: path("Library/Preferences/com.googlecode.iterm2.plist")
            ),
        ]
    }

    static func detected() -> [ImportableTerminal] {
        candidates().filter(\.isDetected)
    }

    static func read(_ terminal: ImportableTerminal) -> ImportedSettings {
        if terminal.id == "iterm2" {
            return readITerm2(at: terminal.configPath)
        }
        guard let text = try? String(contentsOf: terminal.configPath, encoding: .utf8) else {
            return ImportedSettings()
        }
        return parse(text, format: terminal.id)
    }

    static func parse(_ text: String, format: String) -> ImportedSettings {
        switch format {
        case "alacritty":
            return parseAlacritty(text)
        case "kitty":
            return parseKitty(text)
        case "ghostty":
            return parseGhostty(text)
        default:
            return ImportedSettings()
        }
    }

    private static func readITerm2(at url: URL) -> ImportedSettings {
        guard let data = try? Data(contentsOf: url) else {
            return ImportedSettings()
        }
        return parseITerm2Data(data)
    }

    static func parseITerm2Data(_ data: Data) -> ImportedSettings {
        guard let preferences = try? PropertyListDecoder().decode(ITerm2Preferences.self, from: data) else {
            return ImportedSettings()
        }
        return parseITerm2(preferences)
    }

    /// iTerm2 stores colors per profile under `New Bookmarks`; pick the default
    /// profile (by `Default Bookmark Guid`) and convert its color components.
    private static func parseITerm2(_ plist: ITerm2Preferences) -> ImportedSettings {
        var result = ImportedSettings()
        let bookmarks = plist.newBookmarks
        guard !bookmarks.isEmpty else {
            return result
        }
        let profile = plist.defaultBookmarkGuid
            .flatMap { defaultGuid in bookmarks.first { $0.guid == defaultGuid } }
            ?? bookmarks[0]
        result.colors = iTerm2Colors(from: profile)
        return result
    }

    private static func iTerm2Colors(from profile: ITerm2Profile) -> [String: String] {
        var colors: [String: String] = [:]
        func add(_ color: ITerm2Color?, _ termyKey: String) {
            if let color, let hex = iTerm2Hex(color) {
                colors[termyKey] = hex
            }
        }
        add(profile.foreground, "foreground")
        add(profile.background, "background")
        add(profile.cursor, "cursor")
        for (termyKey, color) in zip(ansiColorNames, profile.ansiColors) {
            add(color, termyKey)
        }
        return colors
    }

    private static func iTerm2Hex(_ color: ITerm2Color) -> String? {
        func channel(_ value: Double) -> Int {
            max(0, min(255, Int((value * 255).rounded())))
        }
        return String(
            format: "#%02x%02x%02x",
            channel(color.red),
            channel(color.green),
            channel(color.blue)
        )
    }

    /// Apply imported settings to the Termy config, returning the keys written.
    @MainActor
    @discardableResult
    static func apply(_ settings: ImportedSettings) -> [String] {
        var written: [String] = []
        var failed: [String] = []
        for (key, value) in settings.rootValues {
            do {
                try SettingsBridge.setRoot(key: key, value: value)
                written.append(key)
            } catch {
                failed.append(key)
            }
        }
        for (key, hex) in settings.colors {
            do {
                try SettingsBridge.setColor(key: key, hex: hex)
                written.append(key)
            } catch {
                failed.append(key)
            }
        }
        // Merge only keybinds whose trigger isn't already bound, so existing
        // bindings are never overwritten.
        if !settings.keybinds.isEmpty {
            let existing = TermyConfigurationStore.shared.configuration.keybinds
            let existingTriggers = Set(existing.map { TerminalKeybindConflicts.normalize($0.trigger) })
            let additions = settings.keybinds.filter {
                !existingTriggers.contains(TerminalKeybindConflicts.normalize($0.trigger))
            }
            if !additions.isEmpty {
                let directives = (existing + additions.map { TermyKeybindConfiguration(trigger: $0.trigger, action: $0.action) })
                    .map { "\($0.trigger)=\($0.action)" }
                    .joined(separator: "\n")
                do {
                    try SettingsBridge.setKeybinds(directives)
                    written.append("keybinds")
                } catch {
                    failed.append("keybinds")
                }
            }
        }
        if !written.isEmpty {
            TermyConfigurationStore.shared.reload()
            NotificationCenter.default.post(name: .termySettingsChanged, object: nil)
            TermyToastCenter.shared.show(
                "Imported \(written.count) setting\(written.count == 1 ? "" : "s")",
                kind: .success
            )
        }
        if !failed.isEmpty {
            TermyErrorPresenter.present(
                "Couldn't import some settings",
                message: "Failed to write: \(failed.joined(separator: ", "))"
            )
        }
        return written
    }

    // MARK: - Parsers

    private static func parseAlacritty(_ text: String) -> ImportedSettings {
        // TOML: `family = "JetBrains Mono"` (under [font.normal]) and `size = 14`.
        var result = ImportedSettings()
        if let family = firstMatch(in: text, pattern: #"(?m)^\s*family\s*=\s*"([^"]+)""#) {
            result.fontFamily = family
        }
        if let size = firstMatch(in: text, pattern: #"(?m)^\s*size\s*=\s*([0-9]+(?:\.[0-9]+)?)"#) {
            result.fontSize = Double(size)
        }
        result.colors = parseAlacrittyColors(text)
        return result
    }

    private static func parseKitty(_ text: String) -> ImportedSettings {
        // Space-separated: `font_family JetBrains Mono`, `font_size 14.0`.
        var result = ImportedSettings()
        if let family = firstMatch(in: text, pattern: #"(?m)^\s*font_family\s+(.+?)\s*$"#) {
            result.fontFamily = family
        }
        if let size = firstMatch(in: text, pattern: #"(?m)^\s*font_size\s+([0-9]+(?:\.[0-9]+)?)"#) {
            result.fontSize = Double(size)
        }
        result.colors = parseKittyColors(text)
        result.keybinds = parseKittyKeybinds(text)
        return result
    }

    private static func parseGhostty(_ text: String) -> ImportedSettings {
        // `key = value`: `font-family = JetBrains Mono`, `font-size = 14`.
        var result = ImportedSettings()
        if let family = firstMatch(in: text, pattern: #"(?m)^\s*font-family\s*=\s*(.+?)\s*$"#) {
            result.fontFamily = family.trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
        }
        if let size = firstMatch(in: text, pattern: #"(?m)^\s*font-size\s*=\s*([0-9]+(?:\.[0-9]+)?)"#) {
            result.fontSize = Double(size)
        }
        result.colors = parseGhosttyColors(text)
        result.keybinds = parseGhosttyKeybinds(text)
        return result
    }

    // MARK: - Keybind parsers (conservative: only high-confidence 1:1 actions)

    private static let kittyKeybindActions: [String: String] = [
        "copy_to_clipboard": "copy",
        "paste_from_clipboard": "paste",
        "new_tab": "new_tab",
        "close_tab": "close_tab",
        "next_tab": "switch_tab_right",
        "previous_tab": "switch_tab_left",
    ]

    private static let ghosttyKeybindActions: [String: String] = [
        "copy_to_clipboard": "copy",
        "paste_from_clipboard": "paste",
        "new_tab": "new_tab",
        "close_tab": "close_tab",
        "close_surface": "close_pane_or_tab",
        "next_tab": "switch_tab_right",
        "previous_tab": "switch_tab_left",
        "new_split:right": "split_pane_vertical",
        "new_split:down": "split_pane_horizontal",
    ]

    private static func parseKittyKeybinds(_ text: String) -> [ImportedKeybind] {
        var result: [ImportedKeybind] = []
        for rawLine in text.split(separator: "\n") {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            guard line.hasPrefix("map ") else {
                continue
            }
            let rest = line.dropFirst(4).split(separator: " ", maxSplits: 1, omittingEmptySubsequences: true)
            guard rest.count == 2,
                  let trigger = normalizeTrigger(String(rest[0])),
                  let action = kittyKeybindActions[rest[1].trimmingCharacters(in: .whitespaces)]
            else {
                continue
            }
            result.append(ImportedKeybind(trigger: trigger, action: action))
        }
        return result
    }

    private static func parseGhosttyKeybinds(_ text: String) -> [ImportedKeybind] {
        var result: [ImportedKeybind] = []
        for rawLine in text.split(separator: "\n") {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            guard line.hasPrefix("keybind") else {
                continue
            }
            let parts = line.split(separator: "=", maxSplits: 1)
            guard parts.count == 2 else {
                continue
            }
            let binding = parts[1].trimmingCharacters(in: .whitespaces).split(separator: "=", maxSplits: 1)
            guard binding.count == 2,
                  let trigger = normalizeTrigger(String(binding[0])),
                  let action = ghosttyKeybindActions[binding[1].trimmingCharacters(in: .whitespaces)]
            else {
                continue
            }
            result.append(ImportedKeybind(trigger: trigger, action: action))
        }
        return result
    }

    /// Converts a `+`-separated trigger (`ctrl+shift+c`) to Termy's `-`-separated
    /// form with normalized modifier names (`ctrl-shift-c`).
    static func normalizeTrigger(_ raw: String) -> String? {
        let tokens = raw.split(separator: "+")
            .map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
            .filter { !$0.isEmpty }
        guard !tokens.isEmpty else {
            return nil
        }
        let parts = tokens.map { token -> String in
            switch token {
            case "control", "ctrl": return "ctrl"
            case "command", "cmd", "super": return "cmd"
            case "alt", "opt", "option": return "alt"
            case "shift": return "shift"
            default: return token
            }
        }
        return parts.joined(separator: "-")
    }

    // MARK: - Color parsers

    /// Alacritty groups colors under `[colors.primary|cursor|normal|bright]`
    /// sections, so parsing tracks the active section to disambiguate (e.g.
    /// `black` under `normal` vs `bright`).
    private static func parseAlacrittyColors(_ text: String) -> [String: String] {
        var colors: [String: String] = [:]
        var section = ""
        for rawLine in text.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") {
                continue
            }
            if line.hasPrefix("[") && line.hasSuffix("]") {
                section = String(line.dropFirst().dropLast()).trimmingCharacters(in: .whitespaces)
                continue
            }
            let parts = line.split(separator: "=", maxSplits: 1)
            guard parts.count == 2,
                  let hex = normalizeHex(String(parts[1]))
            else {
                continue
            }
            let key = parts[0].trimmingCharacters(in: .whitespaces)
            switch section {
            case "colors.primary" where key == "foreground" || key == "background":
                colors[key] = hex
            case "colors.cursor" where key == "cursor":
                colors["cursor"] = hex
            case "colors.normal" where normalColorNames.contains(key):
                colors[key] = hex
            case "colors.bright" where normalColorNames.contains(key):
                colors["bright_\(key)"] = hex
            default:
                break
            }
        }
        return colors
    }

    /// Kitty: `foreground #hex`, `background #hex`, `cursor #hex`, `colorN #hex`.
    private static func parseKittyColors(_ text: String) -> [String: String] {
        var colors: [String: String] = [:]
        for rawLine in text.split(separator: "\n", omittingEmptySubsequences: true) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            guard !line.hasPrefix("#") else {
                continue
            }
            let tokens = line.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: true)
            guard tokens.count == 2,
                  let key = mapKittyColorKey(String(tokens[0])),
                  let hex = normalizeHex(String(tokens[1]))
            else {
                continue
            }
            colors[key] = hex
        }
        return colors
    }

    private static func mapKittyColorKey(_ key: String) -> String? {
        switch key {
        case "foreground", "background", "cursor":
            return key
        default:
            guard key.hasPrefix("color"),
                  let index = Int(key.dropFirst("color".count)),
                  ansiColorNames.indices.contains(index)
            else {
                return nil
            }
            return ansiColorNames[index]
        }
    }

    /// Ghostty: `foreground = #hex`, `background = #hex`, `cursor-color = #hex`,
    /// and `palette = N=#hex` for the ANSI entries.
    private static func parseGhosttyColors(_ text: String) -> [String: String] {
        var colors: [String: String] = [:]
        for rawLine in text.split(separator: "\n", omittingEmptySubsequences: true) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            guard !line.hasPrefix("#") else {
                continue
            }
            let parts = line.split(separator: "=", maxSplits: 1)
            guard parts.count == 2 else {
                continue
            }
            let key = parts[0].trimmingCharacters(in: .whitespaces)
            let value = parts[1].trimmingCharacters(in: .whitespaces)
            switch key {
            case "foreground", "background":
                if let hex = normalizeHex(value) {
                    colors[key] = hex
                }
            case "cursor-color":
                if let hex = normalizeHex(value) {
                    colors["cursor"] = hex
                }
            case "palette":
                let entry = value.split(separator: "=", maxSplits: 1)
                if entry.count == 2,
                   let index = Int(entry[0].trimmingCharacters(in: .whitespaces)),
                   ansiColorNames.indices.contains(index),
                   let hex = normalizeHex(String(entry[1])) {
                    colors[ansiColorNames[index]] = hex
                }
            default:
                break
            }
        }
        return colors
    }

    /// Normalizes `#rrggbb`, `0xrrggbb`, or bare `rrggbb` to `#rrggbb`.
    static func normalizeHex(_ raw: String) -> String? {
        var value = raw.trimmingCharacters(in: .whitespaces)
        value = value.trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
        if value.hasPrefix("0x") || value.hasPrefix("0X") {
            value = String(value.dropFirst(2))
        }
        if value.hasPrefix("#") {
            value = String(value.dropFirst())
        }
        guard value.count == 6, value.allSatisfy(\.isHexDigit) else {
            return nil
        }
        return "#" + value.lowercased()
    }

    private static func firstMatch(in text: String, pattern: String) -> String? {
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return nil
        }
        let range = NSRange(text.startIndex..<text.endIndex, in: text)
        guard let match = regex.firstMatch(in: text, range: range),
              match.numberOfRanges > 1,
              let captureRange = Range(match.range(at: 1), in: text)
        else {
            return nil
        }
        return String(text[captureRange]).trimmingCharacters(in: .whitespaces)
    }
}
