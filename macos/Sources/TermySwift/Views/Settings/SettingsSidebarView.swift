import SwiftUI

struct SettingsSidebarView: View {
    let sections: [SettingsSectionModel]
    @Binding var selection: String?

    var body: some View {
        List(selection: $selection) {
            Section {
                ForEach(sections) { section in
                    SettingsSidebarRow(section: section)
                        .tag(section.id as String?)
                }
            }
        }
        .listStyle(.sidebar)
        .navigationSplitViewColumnWidth(min: 176, ideal: 192, max: 230)
    }
}

struct SettingsSidebarRow: View {
    let section: SettingsSectionModel

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            Label(section.label, systemImage: section.systemImage)
                .lineLimit(1)
            if let subtitle = section.subtitle {
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .padding(.leading, 23)
            }
        }
    }
}

extension SettingsSectionModel {
    var subtitle: String? {
        switch id {
        case "appearance": return "Customize the look and feel"
        case "terminal": return "Configure terminal behavior"
        case "tabs": return "Tab bar and split-pane behavior"
        case "themes": return "Browse and install themes"
        case "advanced": return "Power-user options"
        case "colors": return "Per-ANSI color overrides"
        case "keybindings": return "Keyboard shortcuts"
        default: return nil
        }
    }
}
