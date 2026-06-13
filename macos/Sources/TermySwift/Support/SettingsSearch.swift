import Foundation

/// One settings entry matched by a search query, tagged with its section for
/// context in the flattened results list.
struct SettingsSearchResult: Identifiable {
    let sectionLabel: String
    let setting: Setting

    var id: String { "\(sectionLabel):\(setting.key)" }
}

/// Full-settings search across the schema's grouped settings.
enum SettingsSearch {
    static func matches(_ setting: Setting, query: String) -> Bool {
        let needle = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !needle.isEmpty else {
            return true
        }
        return setting.title.lowercased().contains(needle)
            || setting.description.lowercased().contains(needle)
            || setting.key.lowercased().contains(needle)
    }

    /// Flattens the grouped settings of `sections` and returns those matching
    /// `query`, preserving section/group order.
    static func results(in sections: [SettingsSectionModel], query: String) -> [SettingsSearchResult] {
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else {
            return []
        }
        return sections.flatMap { section in
            (section.groups ?? [])
                .flatMap(\.settings)
                .filter { matches($0, query: query) }
                .map { SettingsSearchResult(sectionLabel: section.label, setting: $0) }
        }
    }
}
