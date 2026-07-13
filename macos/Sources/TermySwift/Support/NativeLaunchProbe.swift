import AppKit
import Foundation
import SwiftUI

/// Test-only launch handshake enabled by `TERMY_LAUNCH_PROBE_FILE`.
///
/// The release gate launches the real app with an isolated home and waits for
/// this marker. Recording is requested by a terminal surface only after it has
/// presented a valid frame, then additionally requires a visible AppKit window.
@MainActor
enum NativeLaunchProbe {
    private static let environmentKey = "TERMY_LAUNCH_PROBE_FILE"

    static func recordWhenUsable(_ window: NSWindow) {
        guard let rawPath = ProcessInfo.processInfo.environment[environmentKey],
              !rawPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            return
        }

        let outputURL = URL(fileURLWithPath: rawPath)
        Task { @MainActor in
            for _ in 0..<100 {
                if writeProbeIfUsable(window, to: outputURL) {
                    return
                }
                try? await Task.sleep(nanoseconds: 20_000_000)
            }
        }
    }

    private static func writeProbeIfUsable(_ window: NSWindow, to outputURL: URL) -> Bool {
        guard !FileManager.default.fileExists(atPath: outputURL.path),
              window.isVisible,
              window.windowNumber > 0,
              let contentView = window.contentView,
              contentView.bounds.width > 0,
              contentView.bounds.height > 0
        else {
            return false
        }

        let contents = """
        pid=\(ProcessInfo.processInfo.processIdentifier)
        visible=true
        terminal_ready=true
        window_number=\(window.windowNumber)
        content_width=\(Int(contentView.bounds.width.rounded()))
        content_height=\(Int(contentView.bounds.height.rounded()))
        """

        do {
            try FileManager.default.createDirectory(
                at: outputURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try contents.write(to: outputURL, atomically: true, encoding: .utf8)
            return true
        } catch {
            return false
        }
    }
}

/// Bridges terminal readiness to the containing AppKit window. A plain window
/// geometry check let a fully transparent tmux workspace pass the launch gate.
struct NativeLaunchProbeView: NSViewRepresentable {
    let terminalReady: Bool

    func makeNSView(context: Context) -> NSView {
        NativeLaunchProbeHostingView()
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        (nsView as? NativeLaunchProbeHostingView)?.terminalReady = terminalReady
    }
}

@MainActor
private final class NativeLaunchProbeHostingView: NSView {
    var terminalReady = false {
        didSet {
            recordIfReady()
        }
    }

    private var didScheduleProbe = false

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        recordIfReady()
    }

    private func recordIfReady() {
        guard terminalReady, !didScheduleProbe, let window else {
            return
        }
        didScheduleProbe = true
        NativeLaunchProbe.recordWhenUsable(window)
    }
}
