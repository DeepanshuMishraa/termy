import AppKit

/// Checks GitHub Releases for a newer version and opens the release page to
/// download it. There is no in-place install (that would need a Sparkle-style
/// framework), so the user finishes the update manually.
@MainActor
final class AppUpdater {
    static let shared = AppUpdater()

    typealias DataLoader = @Sendable (URLRequest) async throws -> (Data, URLResponse)

    private let endpoint: URL
    private let dataLoader: DataLoader
    private var isChecking = false

    init(
        endpoint: URL = URL(string: "https://api.github.com/repos/lassejlv/termy/releases/latest")!,
        dataLoader: @escaping DataLoader = { request in
            try await URLSession.shared.data(for: request)
        }
    ) {
        self.endpoint = endpoint
        self.dataLoader = dataLoader
    }

    struct Release: Equatable {
        let version: String
        let url: URL
    }

    enum UpdateError: Error, Equatable {
        case invalidResponse
        case invalidVersion
    }

    func checkForUpdates(userInitiated: Bool) async {
        guard !isChecking else {
            return
        }
        isChecking = true
        defer { isChecking = false }

        do {
            let release = try await fetchLatest()
            if Self.isNewer(release.version, than: currentVersion()) {
                presentUpdateAvailable(release)
            } else if userInitiated {
                presentUpToDate()
            }
        } catch {
            TermyNativeLog.lifecycle.error(
                "Update check failed: \(String(reflecting: type(of: error)), privacy: .public)"
            )
            if userInitiated {
                presentError(error)
            }
        }
    }

    func fetchLatest() async throws -> Release {
        var request = URLRequest(url: endpoint)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        let (data, response) = try await dataLoader(request)
        return try Self.release(from: data, response: response)
    }

    static func release(from data: Data, response: URLResponse) throws -> Release {
        guard let response = response as? HTTPURLResponse,
              (200..<300).contains(response.statusCode)
        else {
            throw UpdateError.invalidResponse
        }
        let decoded = try JSONDecoder().decode(GitHubRelease.self, from: data)
        let version = decoded.tagName.trimmingCharacters(in: CharacterSet(charactersIn: "vV"))
        guard AppVersion(version) != nil else {
            throw UpdateError.invalidVersion
        }
        guard let url = URL(string: decoded.htmlURL),
              url.scheme == "https",
              url.host == "github.com"
        else {
            throw UpdateError.invalidResponse
        }
        return Release(version: version, url: url)
    }

    private func currentVersion() -> String {
        (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String) ?? "0.0.0"
    }

    static func isNewer(_ candidate: String, than current: String) -> Bool {
        guard let candidate = AppVersion(candidate), let current = AppVersion(current) else {
            return false
        }
        return candidate > current
    }

    private func presentUpdateAvailable(_ release: Release) {
        let alert = NSAlert()
        alert.messageText = "Update available"
        alert.informativeText = "Termy \(release.version) is available. You're on \(currentVersion())."
        alert.addButton(withTitle: "Download")
        alert.addButton(withTitle: "Later")
        if alert.runModal() == .alertFirstButtonReturn {
            NSWorkspace.shared.open(release.url)
        }
    }

    private func presentUpToDate() {
        let alert = NSAlert()
        alert.messageText = "You're up to date"
        alert.informativeText = "Termy \(currentVersion()) is the latest version."
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private func presentError(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = "Couldn't check for updates"
        alert.informativeText = String(describing: error)
        alert.alertStyle = .warning
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}

private struct AppVersion: Comparable {
    private enum Identifier: Comparable {
        case number(Int)
        case text(String)

        static func < (lhs: Identifier, rhs: Identifier) -> Bool {
            switch (lhs, rhs) {
            case let (.number(lhs), .number(rhs)):
                lhs < rhs
            case (.number, .text):
                true
            case (.text, .number):
                false
            case let (.text(lhs), .text(rhs)):
                lhs < rhs
            }
        }
    }

    private let core: [Int]
    private let prerelease: [Identifier]?

    init?(_ rawValue: String) {
        let value = rawValue.split(separator: "+", maxSplits: 1, omittingEmptySubsequences: false)[0]
        let parts = value.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let coreParts = parts[0].split(separator: ".", omittingEmptySubsequences: false)
        guard (1...3).contains(coreParts.count),
              coreParts.allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) })
        else {
            return nil
        }
        core = coreParts.map { Int($0)! } + Array(repeating: 0, count: 3 - coreParts.count)

        guard parts.count == 2 else {
            prerelease = nil
            return
        }
        let identifiers = parts[1].split(separator: ".", omittingEmptySubsequences: false)
        guard !identifiers.isEmpty,
              identifiers.allSatisfy({ identifier in
                  !identifier.isEmpty && identifier.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" }
              })
        else {
            return nil
        }
        prerelease = identifiers.map { identifier in
            Int(identifier).map(Identifier.number) ?? .text(String(identifier))
        }
    }

    static func < (lhs: AppVersion, rhs: AppVersion) -> Bool {
        if lhs.core != rhs.core {
            return lhs.core.lexicographicallyPrecedes(rhs.core)
        }
        switch (lhs.prerelease, rhs.prerelease) {
        case (nil, nil):
            return false
        case (nil, _):
            return false
        case (_, nil):
            return true
        case let (lhs?, rhs?):
            for index in 0..<min(lhs.count, rhs.count) where lhs[index] != rhs[index] {
                return lhs[index] < rhs[index]
            }
            return lhs.count < rhs.count
        }
    }
}

private struct GitHubRelease: Decodable {
    let tagName: String
    let htmlURL: String

    enum CodingKeys: String, CodingKey {
        case tagName = "tag_name"
        case htmlURL = "html_url"
    }
}
