import SwiftUI

struct ThemeSettingsContent: View {
    let section: SettingsSectionModel
    @ObservedObject var store: SettingsStore

    private var settingsByKey: [String: Setting] {
        Dictionary(uniqueKeysWithValues: (section.groups ?? [])
            .flatMap(\.settings)
            .map { ($0.key, $0) })
    }

    private var themeChoices: [SettingEnumChoice] {
        settingsByKey["theme"]?.choices ?? []
    }

    private var filteredThemeChoices: [SettingEnumChoice] {
        let choices = themeChoices.filter { $0.value != "shell-decide" }
        return choices.isEmpty ? themeChoices : choices
    }

    private func selectableChoices(for setting: Setting) -> [SettingEnumChoice] {
        let choices = setting.choices ?? []
        return choices.filter { choice in
            choice.installed == true || choice.value == setting.value
        }
    }

    var body: some View {
        Section("Mode") {
            if let mode = settingsByKey["theme_mode"] {
                Picker(selection: store.enumBinding(mode.key)) {
                    ForEach(mode.choices ?? []) { choice in
                        Text(choice.label).tag(choice.value)
                    }
                } label: {
                    SettingLabelView(setting: mode)
                }
                .pickerStyle(.segmented)
            }
        }

        Section("Active Theme") {
            if let theme = settingsByKey["theme"] {
                Picker(selection: store.enumBinding(theme.key)) {
                    ForEach(selectableChoices(for: theme)) { choice in
                        Text(choice.label).tag(choice.value)
                    }
                } label: {
                    SettingLabelView(setting: theme)
                }
            }
        }

        Section("System Appearance") {
            if let light = settingsByKey["theme_light"] {
                Picker(selection: store.enumBinding(light.key)) {
                    ForEach(selectableChoices(for: light)) { choice in
                        Text(choice.label).tag(choice.value)
                    }
                } label: {
                    SettingLabelView(setting: light)
                }
            }

            if let dark = settingsByKey["theme_dark"] {
                Picker(selection: store.enumBinding(dark.key)) {
                    ForEach(selectableChoices(for: dark)) { choice in
                        Text(choice.label).tag(choice.value)
                    }
                } label: {
                    SettingLabelView(setting: dark)
                }
            }
        }

        Section("Available Themes") {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 180), spacing: 10)], spacing: 10) {
                ForEach(filteredThemeChoices) { choice in
                    ThemeChoiceButton(
                        choice: choice,
                        isSelected: store.value(for: "theme") == choice.value,
                        isInstalling: store.installingThemeIDs.contains(choice.value),
                        onSelect: {
                            store.installThemeAndCommitRoot(choice: choice, key: "theme")
                        }
                    )
                }
            }
            .padding(.vertical, 2)
        }
    }
}

struct ThemeChoiceButton: View {
    let choice: SettingEnumChoice
    let isSelected: Bool
    let isInstalling: Bool
    let onSelect: () -> Void

    private var isInstalled: Bool {
        choice.installed ?? false
    }

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 10) {
                ThemeSwatch(hexColors: choice.swatches ?? [])
                VStack(alignment: .leading, spacing: 2) {
                    Text(choice.label)
                        .lineLimit(1)
                    Text(choice.value)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
                if isInstalling {
                    ProgressView()
                        .controlSize(.small)
                } else if isSelected {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(.tint)
                } else if !isInstalled {
                    Text("Install")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, minHeight: 58, alignment: .leading)
            .contentShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .disabled(isInstalling)
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .stroke(isSelected ? Color.accentColor : Color.secondary.opacity(0.25), lineWidth: isSelected ? 1.5 : 1)
        }
    }
}

struct ThemeSwatch: View {
    let hexColors: [String]

    private var colors: [Color] {
        let parsed = hexColors.compactMap(Color.init(hex:))
        if parsed.isEmpty {
            return [
                Color(nsColor: .controlAccentColor),
                Color(nsColor: .secondaryLabelColor),
                Color(nsColor: .tertiaryLabelColor),
                Color(nsColor: .quaternaryLabelColor),
            ]
        }
        return Array(parsed.prefix(6))
    }

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(colors.enumerated()), id: \.offset) { _, color in
                color
                    .frame(width: max(5, 32 / CGFloat(colors.count)))
            }
        }
        .frame(width: 32, height: 32)
        .clipShape(RoundedRectangle(cornerRadius: 5))
        .overlay {
            RoundedRectangle(cornerRadius: 5)
                .stroke(Color.black.opacity(0.15), lineWidth: 1)
        }
    }
}
