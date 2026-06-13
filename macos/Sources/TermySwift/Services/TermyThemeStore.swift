import Foundation

/// One theme from the public registry index, matching `ThemeRegistryEntry` in
/// `crates/theme_core`.
struct ThemeStoreEntry: Identifiable, Decodable, Equatable {
    var name: String
    var slug: String
    var latestVersion: String
    var description: String?

    var id: String { slug }
}

/// Fetches the public Termy theme registry. Installing a chosen theme reuses the
/// core's `termy_settings_install_theme`, which downloads and installs by slug.
@MainActor
final class TermyThemeStore: ObservableObject {
    enum LoadState: Equatable {
        case loading
        case loaded([ThemeStoreEntry])
        case failed(String)
    }

    static let registryURL = URL(
        string: "https://raw.githubusercontent.com/termy-org/themes/main/index.json"
    )!

    @Published private(set) var state: LoadState = .loading

    func load() async {
        state = .loading
        do {
            let (data, _) = try await URLSession.shared.data(from: Self.registryURL)
            state = .loaded(try Self.parseIndex(data))
        } catch {
            state = .failed(String(describing: error))
        }
    }

    nonisolated static func parseIndex(_ data: Data) throws -> [ThemeStoreEntry] {
        struct Index: Decodable {
            var themes: [ThemeStoreEntry]
        }
        return try JSONDecoder().decode(Index.self, from: data).themes
    }
}
