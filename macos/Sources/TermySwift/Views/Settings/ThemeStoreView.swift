import SwiftUI

/// Browses the public theme registry and installs a chosen theme.
struct ThemeStoreView: View {
    @ObservedObject var settingsStore: SettingsStore
    @StateObject private var store = TermyThemeStore()
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Theme Store")
                    .font(.headline)
                Spacer()
                Button("Done") { dismiss() }
            }
            .padding()

            Divider()

            content
        }
        .frame(width: 460, height: 520)
        .task {
            await store.load()
        }
    }

    @ViewBuilder
    private var content: some View {
        switch store.state {
        case .loading:
            ProgressView("Loading themes…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            ContentUnavailableView(
                "Couldn't load the theme store",
                systemImage: "wifi.slash",
                description: Text(message)
            )
        case .loaded(let themes):
            List(themes) { theme in
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(theme.name)
                            .font(.body.weight(.medium))
                        if let description = theme.description, !description.isEmpty {
                            Text(description)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                    }
                    Spacer()
                    if settingsStore.installingThemeIDs.contains(theme.slug) {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Button("Install") {
                            settingsStore.installStoreTheme(slug: theme.slug)
                        }
                    }
                }
                .padding(.vertical, 2)
            }
        }
    }
}
