import AppKit
import Darwin
import Foundation
import OSLog
import UniformTypeIdentifiers

struct TermyDiagnosticsLogEntry: Equatable {
    var date: Date
    var category: String
    var level: String
    var message: String
}

struct TermyDiagnosticsSnapshot {
    var generatedAt: Date
    var appVersion: String
    var buildVersion: String
    var bundleIdentifier: String
    var architecture: String
    var operatingSystem: String
    var configPath: String?
    var configDiagnostics: [TermyConfigDiagnostic]
    var recentLogs: [TermyDiagnosticsLogEntry]
    var logCollectionError: String?
}

enum TermyDiagnosticsReport {
    static func render(_ snapshot: TermyDiagnosticsSnapshot, homeDirectory: String) -> String {
        let timestamp = ISO8601DateFormatter().string(from: snapshot.generatedAt)
        var lines = [
            "Termy diagnostics",
            "Generated: \(timestamp)",
            "Version: \(sanitize(snapshot.appVersion, homeDirectory: homeDirectory))",
            "Build: \(sanitize(snapshot.buildVersion, homeDirectory: homeDirectory))",
            "Bundle: \(sanitize(snapshot.bundleIdentifier, homeDirectory: homeDirectory))",
            "Architecture: \(sanitize(snapshot.architecture, homeDirectory: homeDirectory))",
            "macOS: \(sanitize(snapshot.operatingSystem, homeDirectory: homeDirectory))",
            "Config: \(sanitizePath(snapshot.configPath, homeDirectory: homeDirectory))",
            "",
            "Configuration diagnostics (\(snapshot.configDiagnostics.count))",
        ]

        if snapshot.configDiagnostics.isEmpty {
            lines.append("(none)")
        } else {
            lines.append(contentsOf: snapshot.configDiagnostics.map { diagnostic in
                let location = diagnostic.lineNumber > 0 ? "line \(diagnostic.lineNumber)" : "no line"
                return "[\(kindName(diagnostic.kind))] \(location): "
                    + sanitize(diagnostic.message, homeDirectory: homeDirectory)
            })
        }

        lines.append("")
        lines.append("Recent native logs (\(snapshot.recentLogs.count))")
        if let error = snapshot.logCollectionError {
            lines.append("Log collection unavailable: \(sanitize(error, homeDirectory: homeDirectory))")
        } else if snapshot.recentLogs.isEmpty {
            lines.append("(none)")
        } else {
            lines.append(contentsOf: snapshot.recentLogs.map { entry in
                let date = ISO8601DateFormatter().string(from: entry.date)
                return "\(date) [\(sanitize(entry.level, homeDirectory: homeDirectory))] "
                    + "[\(sanitize(entry.category, homeDirectory: homeDirectory))] "
                    + sanitize(entry.message, homeDirectory: homeDirectory)
            })
        }

        lines.append("")
        lines.append("Terminal output, configuration contents, and environment variables are intentionally excluded.")
        return lines.joined(separator: "\n") + "\n"
    }

    static func sanitize(_ value: String, homeDirectory: String) -> String {
        var sanitized = value
        if !homeDirectory.isEmpty {
            sanitized = sanitized.replacingOccurrences(of: homeDirectory, with: "~")
        }

        let patterns = [
            #"(?i)\b(api[_-]?key|access[_-]?token|auth(?:orization)?|cookie|password|secret|token)\b\s*[:=]\s*[^\s,;]+"#,
            #"(?i)(https?://)[^/@:\s]+:[^/@\s]+@"#,
            #"\b(?:ghp|github_pat|sk)-[A-Za-z0-9_-]{8,}\b"#,
        ]
        for pattern in patterns {
            sanitized = sanitized.replacingMatches(of: pattern, with: "<redacted>")
        }
        return sanitized
    }

    private static func sanitizePath(_ path: String?, homeDirectory: String) -> String {
        guard let path, !path.isEmpty else {
            return "(default path unavailable)"
        }
        return sanitize(path, homeDirectory: homeDirectory)
    }

    private static func kindName(_ kind: TermyConfigDiagnostic.Kind) -> String {
        switch kind {
        case .unknown:
            "unknown"
        case .unknownSection:
            "unknown-section"
        case .unknownRootKey:
            "unknown-root-key"
        case .unknownColorKey:
            "unknown-color-key"
        case .invalidSyntax:
            "invalid-syntax"
        case .invalidValue:
            "invalid-value"
        case .duplicateRootKey:
            "duplicate-root-key"
        }
    }
}

enum TermyNativeLog {
    static let lifecycle = Logger(subsystem: AppMetadata.bundleIdentifier, category: "lifecycle")
    static let diagnostics = Logger(subsystem: AppMetadata.bundleIdentifier, category: "diagnostics")
}

@MainActor
final class TermyDiagnosticsExporter {
    static let shared = TermyDiagnosticsExporter()

    private init() {}

    func export() {
        TermyNativeLog.diagnostics.info("Diagnostics export requested")
        let report = TermyDiagnosticsReport.render(
            snapshot(),
            homeDirectory: FileManager.default.homeDirectoryForCurrentUser.path
        )
        let panel = NSSavePanel()
        panel.title = "Export Termy Diagnostics"
        panel.nameFieldStringValue = "Termy-Diagnostics-\(Self.fileTimestamp()).txt"
        panel.allowedContentTypes = [.plainText]
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else {
            return
        }

        do {
            try report.write(to: url, atomically: true, encoding: .utf8)
        } catch {
            let alert = NSAlert(error: error)
            alert.messageText = "Could not export diagnostics"
            alert.runModal()
        }
    }

    private func snapshot() -> TermyDiagnosticsSnapshot {
        let bundle = Bundle.main
        let configuration = TermyConfigurationStore.shared.configuration
        let logs: (entries: [TermyDiagnosticsLogEntry], error: String?)
        do {
            logs = (try Self.recentLogs(), nil)
        } catch {
            logs = ([], String(describing: error))
        }
        return TermyDiagnosticsSnapshot(
            generatedAt: Date(),
            appVersion: bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown",
            buildVersion: bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "unknown",
            bundleIdentifier: bundle.bundleIdentifier ?? AppMetadata.bundleIdentifier,
            architecture: Self.architecture(),
            operatingSystem: ProcessInfo.processInfo.operatingSystemVersionString,
            configPath: configuration.configPath,
            configDiagnostics: configuration.diagnostics,
            recentLogs: logs.entries,
            logCollectionError: logs.error
        )
    }

    private static func recentLogs(now: Date = Date()) throws -> [TermyDiagnosticsLogEntry] {
        let store = try OSLogStore(scope: .currentProcessIdentifier)
        let position = store.position(date: now.addingTimeInterval(-15 * 60))
        let predicate = NSPredicate(format: "subsystem == %@", AppMetadata.bundleIdentifier)
        return Array(try store.getEntries(at: position, matching: predicate)
            .compactMap { entry -> TermyDiagnosticsLogEntry? in
                guard let entry = entry as? OSLogEntryLog else {
                    return nil
                }
                return TermyDiagnosticsLogEntry(
                    date: entry.date,
                    category: entry.category,
                    level: levelName(entry.level),
                    message: entry.composedMessage
                )
            }
            .suffix(200))
    }

    private static func architecture() -> String {
        var systemInfo = utsname()
        uname(&systemInfo)
        return withUnsafePointer(to: &systemInfo.machine) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: 1) {
                String(cString: $0)
            }
        }
    }

    private static func levelName(_ level: OSLogEntryLog.Level) -> String {
        switch level {
        case .undefined:
            "undefined"
        case .debug:
            "debug"
        case .info:
            "info"
        case .notice:
            "notice"
        case .error:
            "error"
        case .fault:
            "fault"
        @unknown default:
            "unknown"
        }
    }

    private static func fileTimestamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        return formatter.string(from: Date())
    }
}

private extension String {
    func replacingMatches(of pattern: String, with replacement: String) -> String {
        guard let expression = try? NSRegularExpression(pattern: pattern) else {
            return self
        }
        let range = NSRange(startIndex..<endIndex, in: self)
        return expression.stringByReplacingMatches(in: self, range: range, withTemplate: replacement)
    }
}
