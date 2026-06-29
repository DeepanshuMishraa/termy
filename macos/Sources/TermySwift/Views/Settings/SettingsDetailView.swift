import SwiftUI

struct SettingsSearchResultsView: View {
    let results: [SettingsSearchResult]
    @ObservedObject var store: SettingsStore

    var body: some View {
        Group {
            if results.isEmpty {
                ContentUnavailableView.search
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(results) { result in
                            VStack(alignment: .leading, spacing: 0) {
                                HStack {
                                    Text(result.setting.title)
                                        .font(.system(size: 11, weight: .medium))
                                        .foregroundStyle(.secondary)
                                    Spacer()
                                    Text(result.sectionLabel)
                                        .font(.system(size: 10))
                                        .foregroundStyle(.tertiary)
                                }
                                .padding(.horizontal, 16)
                                .padding(.top, 8)
                                .padding(.bottom, 2)

                                SettingRow(setting: result.setting, store: store)
                                    .padding(.horizontal, 16)
                                    .padding(.bottom, 8)
                            }
                            Divider()
                        }
                    }
                    .padding(.vertical, 4)
                }
            }
        }
        .frame(minWidth: 500, maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct SettingsDetailView: View {
    let section: SettingsSectionModel?
    @ObservedObject var store: SettingsStore

    var body: some View {
        Group {
            if let section {
                SettingsSectionView(section: section, store: store)
            } else {
                ContentUnavailableView(
                    "Settings",
                    systemImage: "gearshape",
                    description: Text("No supported settings are available.")
                )
            }
        }
        .frame(minWidth: 500, maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct SettingsSectionView: View {
    let section: SettingsSectionModel
    @ObservedObject var store: SettingsStore

    var body: some View {
        Form {
            if section.id == SettingsSectionModel.themesSectionID {
                ThemeSettingsContent(section: section, store: store)
            } else if let colors = section.colors {
                ColorSettingsContent(colors: colors, store: store)
            } else if section.keybinds != nil {
                KeybindSettingsContent(store: store)
            } else {
                ForEach(section.groups ?? []) { group in
                    Section(SettingsGroupLabel.format(group.label)) {
                        ForEach(group.settings) { setting in
                            SettingRow(setting: setting, store: store)
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(section.label)
    }
}

enum SettingsGroupLabel {
    static func format(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed == trimmed.uppercased(), trimmed != trimmed.lowercased() else {
            return raw
        }
        return trimmed.capitalized
    }
}
