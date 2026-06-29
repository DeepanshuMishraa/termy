import SwiftUI

@MainActor
final class SettingsStore: ObservableObject {
    @Published private(set) var schema: SettingsSchema?
    @Published var errorMessage: String?

    /// Canonical current values, keyed by setting key. Edited optimistically and
    /// written straight through to the config file via `SettingsBridge`.
    @Published private(set) var values: [String: String] = [:]
    /// Color overrides keyed by color key. Empty string means "inherit theme".
    @Published private(set) var colors: [String: String] = [:]
    @Published var keybindsText: String = ""
    @Published private(set) var installingThemeIDs: Set<String> = []

    func load() {
        do {
            let schema = try SettingsBridge.loadSchema()
            var values: [String: String] = [:]
            var colors: [String: String] = [:]
            for section in schema.sections {
                for group in section.groups ?? [] {
                    for setting in group.settings {
                        values[setting.key] = setting.value ?? ""
                    }
                }
                for color in section.colors ?? [] {
                    colors[color.key] = color.hex ?? ""
                }
                if let keybinds = section.keybinds {
                    keybindsText = keybinds.joined(separator: "\n")
                }
            }
            self.values = values
            self.colors = colors
            self.schema = schema
            errorMessage = nil
        } catch {
            report(error)
        }
    }

    func section(id: String?) -> SettingsSectionModel? {
        guard let id else {
            return nil
        }
        return schema?.sections.first { $0.id == id }
    }

    func value(for key: String) -> String {
        values[key] ?? ""
    }

    func commitRoot(key: String, value: String) {
        values[key] = value
        commit {
            if Self.shouldResetRootSetting(key: key, value: value) {
                try SettingsBridge.resetRoot(key: key)
            } else {
                try SettingsBridge.setRoot(key: key, value: value)
            }
        }
    }

    func installThemeAndCommitRoot(choice: SettingEnumChoice, key: String) {
        let slug = choice.value
        guard choice.installed != true, !installingThemeIDs.contains(slug) else {
            commitRoot(key: key, value: slug)
            return
        }

        installingThemeIDs.insert(slug)
        Task { @MainActor in
            do {
                try await Task.detached(priority: .userInitiated) {
                    try SettingsBridge.installTheme(slug: slug)
                }.value
                installingThemeIDs.remove(slug)
                load()
                commitRoot(key: key, value: slug)
            } catch {
                installingThemeIDs.remove(slug)
                report(error)
            }
        }
    }

    /// Installs a theme chosen from the remote store (by slug) and makes it the
    /// active theme.
    func installStoreTheme(slug: String) {
        guard !installingThemeIDs.contains(slug) else {
            return
        }
        installingThemeIDs.insert(slug)
        Task { @MainActor in
            do {
                try await Task.detached(priority: .userInitiated) {
                    try SettingsBridge.installTheme(slug: slug)
                }.value
                installingThemeIDs.remove(slug)
                load()
                commitRoot(key: "theme", value: slug)
                TermyToastCenter.shared.show("Installed theme \(slug)", kind: .success)
            } catch {
                installingThemeIDs.remove(slug)
                report(error)
            }
        }
    }

    func boolBinding(_ key: String) -> Binding<Bool> {
        Binding(
            get: { self.values[key] == "true" },
            set: { self.commitRoot(key: key, value: $0 ? "true" : "false") }
        )
    }

    func enumBinding(_ key: String) -> Binding<String> {
        Binding(
            get: { self.values[key] ?? "" },
            set: { self.commitRoot(key: key, value: $0) }
        )
    }

    func colorHex(for key: String) -> String {
        colors[key] ?? ""
    }

    func commitColor(key: String, hex: String?) {
        colors[key] = hex ?? ""
        commit {
            try SettingsBridge.setColor(key: key, hex: hex)
        }
    }

    func commitKeybinds() {
        commit {
            try SettingsBridge.setKeybinds(keybindsText)
        }
    }

    func resetSetting(key: String) {
        values.removeValue(forKey: key)
        commit {
            try SettingsBridge.resetRoot(key: key)
        }
    }

    // MARK: - Structured keybind management

    struct KeybindEntry: Identifiable, Equatable {
        var trigger: String
        var action: String
        var id: String { "\(trigger)=\(action)" }
    }

    var keybindEntries: [KeybindEntry] {
        keybindsText
            .components(separatedBy: "\n")
            .filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
            .map { line in
                let parts = line.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                let trigger = parts.first.map { String($0).trimmingCharacters(in: .whitespaces) } ?? ""
                let action = parts.count > 1 ? String(parts[1]).trimmingCharacters(in: .whitespaces) : ""
                return KeybindEntry(trigger: trigger, action: action)
            }
    }

    func updateKeybindTrigger(action: String, oldTrigger: String, newTrigger: String) {
        var entries = keybindEntries
        if let index = entries.firstIndex(where: { $0.trigger == oldTrigger && $0.action == action }) {
            entries[index] = KeybindEntry(trigger: newTrigger, action: action)
        } else {
            entries.append(KeybindEntry(trigger: newTrigger, action: action))
        }
        keybindsText = entries.map { "\($0.trigger)=\($0.action)" }.joined(separator: "\n")
        commitKeybinds()
    }

    func deleteKeybind(trigger: String, action: String) {
        var entries = keybindEntries
        entries.removeAll { $0.trigger == trigger && $0.action == action }
        keybindsText = entries.map { "\($0.trigger)=\($0.action)" }.joined(separator: "\n")
        commitKeybinds()
    }

    func addKeybind(trigger: String, action: String) {
        var entries = keybindEntries
        guard !entries.contains(where: { $0.trigger == trigger && $0.action == action }) else {
            return
        }
        entries.append(KeybindEntry(trigger: trigger, action: action))
        keybindsText = entries.map { "\($0.trigger)=\($0.action)" }.joined(separator: "\n")
        commitKeybinds()
    }

    private func commit(_ write: () throws -> Void) {
        do {
            try write()
            notifyChanged()
        } catch {
            report(error)
        }
    }

    private func notifyChanged() {
        TermyConfigurationStore.shared.reload()
        NotificationCenter.default.post(name: .termySettingsChanged, object: nil)
    }

    private func report(_ error: Error) {
        errorMessage = String(describing: error)
    }

    private static func shouldResetRootSetting(key: String, value: String) -> Bool {
        guard value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return false
        }
        return [
            "working_dir",
            "shell",
            "colorterm",
            "inactive_tab_scrollback",
        ].contains(key)
    }
}
