import AppKit
import Darwin
import Foundation

struct NativeSoakReport: Codable, Equatable {
    var durationSeconds: Double
    var cycles: Int
    var outputLines: Int
    var tabsOpened: Int
    var initialRSSMiB: Double
    var maximumRSSMiB: Double
    var finalRSSMiB: Double
    var rssGrowthMiB: Double
    var initialWindowCount: Int
    var maximumWindowCount: Int
    var finalWindowCount: Int
    var maximumPaneCount: Int
    var finalPaneCount: Int
    var errors: [String]
}

@MainActor
final class NativeSoakRunner {
    static let shared = NativeSoakRunner()
    static var isRequested: Bool {
        ProcessInfo.processInfo.environment["TERMY_NATIVE_SOAK_DURATION_SECS"] != nil
    }

    private var task: Task<Void, Never>?

    private init() {}

    func startIfRequested() {
        guard task == nil,
              let rawDuration = ProcessInfo.processInfo.environment["TERMY_NATIVE_SOAK_DURATION_SECS"],
              let duration = Double(rawDuration), duration > 0,
              let reportPath = ProcessInfo.processInfo.environment["TERMY_NATIVE_SOAK_REPORT_FILE"],
              !reportPath.isEmpty
        else {
            return
        }
        task = Task { @MainActor [weak self] in
            await self?.run(duration: duration, reportURL: URL(fileURLWithPath: reportPath))
        }
    }

    private func run(duration: TimeInterval, reportURL: URL) async {
        let ready = await waitUntil(timeout: 15) { !self.terminalWindows().isEmpty }
        guard ready else {
            writeFailure("No terminal window became ready", duration: duration, reportURL: reportURL)
            NSApp.terminate(nil)
            return
        }

        let startedAt = Date()
        let initialWindowIDs = Set(terminalWindows().map { ObjectIdentifier($0.window) })
        let initialWindowCount = initialWindowIDs.count
        let initialRSS = Self.currentResidentMemoryMiB()
        var maximumRSS = initialRSS
        var maximumWindowCount = initialWindowCount
        var maximumPaneCount = totalPaneCount()
        var cycles = 0
        var outputLines = 0
        var tabsOpened = 0
        var errors: [String] = []

        while Date().timeIntervalSince(startedAt) < duration, !Task.isCancelled {
            guard let target = terminalWindows().first else {
                errors.append("All terminal windows disappeared during cycle \(cycles)")
                break
            }

            target.window.makeKeyAndOrderFront(nil)
            let lineCount = 200
            let command = "printf 'termy-soak-\(cycles)-%04d\\n' {1..\(lineCount)}\r"
            target.store.focusedTerminal?.send(bytes: Array(command.utf8))
            outputLines += lineCount

            if target.store.paneCount == 1 {
                target.store.splitFocused(cycles.isMultiple(of: 2) ? .horizontal : .vertical)
            } else {
                _ = target.store.closeFocusedPaneIfSplit()
            }

            target.window.setContentSize(cycles.isMultiple(of: 2)
                ? NSSize(width: 980, height: 640)
                : NSSize(width: 1280, height: 820))

            if cycles.isMultiple(of: 8) {
                let before = Set(terminalWindows().map { ObjectIdentifier($0.window) })
                NativeTabWindowManager.shared.openNativeTab()
                tabsOpened += 1
                try? await Task.sleep(for: .milliseconds(150))
                if let opened = terminalWindows().first(where: { !before.contains(ObjectIdentifier($0.window)) }) {
                    opened.store.focusedTerminal?.send(bytes: Array("printf 'termy-soak-tab-\(cycles)\\n'\r".utf8))
                    try? await Task.sleep(for: .milliseconds(50))
                    opened.window.performClose(nil)
                } else {
                    errors.append("Native tab did not register during cycle \(cycles)")
                }
            }

            cycles += 1
            let windows = terminalWindows()
            maximumWindowCount = max(maximumWindowCount, windows.count)
            maximumPaneCount = max(maximumPaneCount, windows.reduce(0) { $0 + $1.store.paneCount })
            maximumRSS = max(maximumRSS, Self.currentResidentMemoryMiB())
            try? await Task.sleep(for: .milliseconds(250))
        }

        for target in terminalWindows() where !initialWindowIDs.contains(ObjectIdentifier(target.window)) {
            target.window.performClose(nil)
        }
        for target in terminalWindows() {
            while target.store.closeFocusedPaneIfSplit() {}
        }
        try? await Task.sleep(for: .milliseconds(250))

        let finalWindows = terminalWindows()
        let finalRSS = Self.currentResidentMemoryMiB()
        maximumRSS = max(maximumRSS, finalRSS)
        if finalWindows.count > initialWindowCount {
            errors.append("Terminal window count grew from \(initialWindowCount) to \(finalWindows.count)")
        }

        write(NativeSoakReport(
            durationSeconds: Date().timeIntervalSince(startedAt),
            cycles: cycles,
            outputLines: outputLines,
            tabsOpened: tabsOpened,
            initialRSSMiB: initialRSS,
            maximumRSSMiB: maximumRSS,
            finalRSSMiB: finalRSS,
            rssGrowthMiB: finalRSS - initialRSS,
            initialWindowCount: initialWindowCount,
            maximumWindowCount: maximumWindowCount,
            finalWindowCount: finalWindows.count,
            maximumPaneCount: maximumPaneCount,
            finalPaneCount: finalWindows.reduce(0) { $0 + $1.store.paneCount },
            errors: errors
        ), to: reportURL)
        NSApp.terminate(nil)
    }

    private func terminalWindows() -> [(window: NSWindow, store: TerminalWorkspaceStore)] {
        NSApp.windows.compactMap { window in
            TerminalCommandRouter.shared.store(forWindow: window).map { (window, $0) }
        }
    }

    private func totalPaneCount() -> Int {
        terminalWindows().reduce(0) { $0 + $1.store.paneCount }
    }

    private func waitUntil(
        timeout: TimeInterval,
        condition: @escaping @MainActor () -> Bool
    ) async -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() {
                return true
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return condition()
    }

    private func writeFailure(_ message: String, duration: Double, reportURL: URL) {
        let rss = Self.currentResidentMemoryMiB()
        write(NativeSoakReport(
            durationSeconds: duration,
            cycles: 0,
            outputLines: 0,
            tabsOpened: 0,
            initialRSSMiB: rss,
            maximumRSSMiB: rss,
            finalRSSMiB: rss,
            rssGrowthMiB: 0,
            initialWindowCount: 0,
            maximumWindowCount: 0,
            finalWindowCount: 0,
            maximumPaneCount: 0,
            finalPaneCount: 0,
            errors: [message]
        ), to: reportURL)
    }

    private func write(_ report: NativeSoakReport, to url: URL) {
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            try encoder.encode(report).write(to: url, options: .atomic)
        } catch {
            TermyNativeLog.lifecycle.fault(
                "Native soak report write failed: \(String(reflecting: type(of: error)), privacy: .public)"
            )
        }
    }

    private static func currentResidentMemoryMiB() -> Double {
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(
            MemoryLayout<mach_task_basic_info>.stride / MemoryLayout<natural_t>.stride
        )
        let status = withUnsafeMutablePointer(to: &info) { pointer in
            pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { rebound in
                task_info(
                    mach_task_self_,
                    task_flavor_t(MACH_TASK_BASIC_INFO),
                    rebound,
                    &count
                )
            }
        }
        guard status == KERN_SUCCESS else {
            return 0
        }
        return Double(info.resident_size) / 1_048_576
    }
}
