import Foundation

/// Render-plan-build time percentiles (microseconds) over a benchmark run. This
/// is the CPU render cost; GPU/display timing is collected by the windowed
/// xctrace comparison.
struct BenchmarkBuildTimes: Codable, Equatable {
    var samples = 0
    var meanMicros = 0.0
    var p50Micros = 0.0
    var p95Micros = 0.0
    var p99Micros = 0.0

    var encodedJSON: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try! encoder.encode(self)
        return String(decoding: data, as: UTF8.self)
    }
}

struct BenchmarkResult {
    var metrics: NativeRenderMetricsSnapshot
    var buildTimes: BenchmarkBuildTimes
}

struct NativeBenchmarkScenarioResult: Codable, Equatable {
    var scenario: String
    var metrics: NativeRenderMetricsSnapshot
    var buildTimes: BenchmarkBuildTimes

    var encodedJSON: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try! encoder.encode(self)
        return String(decoding: data, as: UTF8.self)
    }
}

enum NativeBenchmarkScenario: String, CaseIterable {
    case idleCursorBlink = "idle-cursor-blink"
    case continuousBulkOutput = "continuous-bulk-output"
    case largeScrollbackNavigation = "large-scrollback-navigation"
    case fullscreenTUIRedraw = "fullscreen-tui-redraw"
    case rapidResize = "rapid-resize"
    case splitPanes2 = "split-panes-2"
    case splitPanes4 = "split-panes-4"
    case splitPanes8 = "split-panes-8"
    case tmuxControlOutput = "tmux-control-output"
    case searchManyMatches = "search-many-matches"

    var paneCount: Int {
        switch self {
        case .splitPanes2: return 2
        case .splitPanes4: return 4
        case .splitPanes8: return 8
        default: return 1
        }
    }
}

/// Deterministic, headless render-pipeline benchmarks. Each scenario drives the
/// real display-terminal -> frame-store -> render-plan path without relying on
/// PTY scheduling, and emits an independent sample set.
enum TermyBenchmarkRunner {
    static let argument = "--benchmark"
    private static let defaultFrames = 120

    private final class Pipeline {
        let terminal: LibTermyTerminal
        let store = TerminalFrameStore()
        let planCache = TerminalRenderPlanCache()

        init(cols: Int, rows: Int) throws {
            terminal = try LibTermyTerminal(
                displayCols: UInt16(cols),
                rows: UInt16(rows),
                loadUserConfig: false
            )
        }
    }

    static func runIfRequested() {
        guard CommandLine.arguments.contains(argument) else {
            return
        }
        do {
            for result in try runAll() {
                print("native-benchmark-result \(result.encodedJSON)")
            }
            exit(0)
        } catch {
            FileHandle.standardError.write(Data("termy benchmark failed: \(error)\n".utf8))
            exit(1)
        }
    }

    static func runAll(cols: Int = 80, rows: Int = 24, frames: Int = defaultFrames) throws -> [NativeBenchmarkScenarioResult] {
        try NativeBenchmarkScenario.allCases.map { scenario in
            let result = try run(scenario: scenario, cols: cols, rows: rows, frames: frames)
            return NativeBenchmarkScenarioResult(
                scenario: scenario.rawValue,
                metrics: result.metrics,
                buildTimes: result.buildTimes
            )
        }
    }

    /// Retained as the focused unit-test seam and as a useful single-workload
    /// entry point for local profiling.
    static func run(cols: Int = 80, rows: Int = 24, frames: Int = defaultFrames) throws -> BenchmarkResult {
        try run(scenario: .continuousBulkOutput, cols: cols, rows: rows, frames: frames)
    }

    static func run(
        scenario: NativeBenchmarkScenario,
        cols: Int = 80,
        rows: Int = 24,
        frames: Int = defaultFrames
    ) throws -> BenchmarkResult {
        precondition(frames > 0)
        let recorder = NativeRenderMetricsRecorder()
        let pipelines = try (0..<scenario.paneCount).map { _ in
            try Pipeline(cols: cols, rows: rows)
        }
        var buildMicros: [Double] = []

        for pipeline in pipelines {
            if let micros = try poll(pipeline, recorder, forceFull: true) {
                buildMicros.append(micros)
            }
        }
        try prepare(scenario, pipelines: pipelines, rows: rows)

        for frame in 0..<frames {
            try mutate(scenario, pipelines: pipelines, frame: frame, cols: cols, rows: rows)
            for pipeline in pipelines {
                let forceFull = ProcessInfo.processInfo.environment["TERMY_BENCHMARK_FORCE_FULL"] == "1"
                if let micros = try poll(pipeline, recorder, forceFull: forceFull) {
                    buildMicros.append(micros)
                }
            }
        }
        return BenchmarkResult(
            metrics: recorder.snapshot,
            buildTimes: percentiles(fromMicros: buildMicros)
        )
    }

    private static func prepare(
        _ scenario: NativeBenchmarkScenario,
        pipelines: [Pipeline],
        rows: Int
    ) throws {
        switch scenario {
        case .largeScrollbackNavigation:
            let history = (0..<2_000)
                .map { "history \($0) benchmark scrollback row\n" }
                .joined()
            try pipelines[0].terminal.feedOutput(Array(history.utf8))
            _ = try pipelines[0].terminal.scrollDisplay(deltaLines: Int32(rows))
        case .searchManyMatches:
            let corpus = (0..<2_000)
                .map { "search target \($0) target target\n" }
                .joined()
            try pipelines[0].terminal.feedOutput(Array(corpus.utf8))
        default:
            break
        }
    }

    private static func mutate(
        _ scenario: NativeBenchmarkScenario,
        pipelines: [Pipeline],
        frame: Int,
        cols: Int,
        rows: Int
    ) throws {
        switch scenario {
        case .idleCursorBlink:
            let row = (frame % max(rows, 2)) + 1
            let visibility = frame.isMultiple(of: 2) ? "h" : "l"
            try pipelines[0].terminal.feedOutput(Array("\u{1B}[?25\(visibility)\u{1B}[\(row);1H".utf8))

        case .continuousBulkOutput:
            let row = (frame % max(rows, 1)) + 1
            let text = "\u{1B}[\(row);1Hbulk \(frame) abcdefghijklmnopqrstuvwxyz 0123456789"
            try pipelines[0].terminal.feedOutput(Array(text.utf8))

        case .largeScrollbackNavigation:
            let delta: Int32 = frame.isMultiple(of: 2) ? 1 : -1
            _ = try pipelines[0].terminal.scrollDisplay(deltaLines: delta)

        case .fullscreenTUIRedraw:
            var screen = "\u{1B}[H"
            for row in 0..<rows {
                let marker = (row + frame) % max(cols, 1)
                screen += String(repeating: ".", count: marker)
                screen += "#"
                screen += String(repeating: " ", count: max(0, cols - marker - 1))
                if row + 1 < rows { screen += "\n" }
            }
            try pipelines[0].terminal.feedOutput(Array(screen.utf8))

        case .rapidResize:
            let width = frame.isMultiple(of: 2) ? cols : max(20, cols - 17)
            let height = frame.isMultiple(of: 2) ? rows : max(8, rows - 5)
            try pipelines[0].terminal.resize(
                cols: UInt16(width),
                rows: UInt16(height),
                cellWidth: 8,
                cellHeight: 16
            )

        case .splitPanes2, .splitPanes4, .splitPanes8:
            for (index, pipeline) in pipelines.enumerated() {
                try pipeline.terminal.feedOutput(Array("pane \(index) frame \(frame) output\n".utf8))
            }

        case .tmuxControlOutput:
            let controlModeOutput = "tmux pane %1 frame \(frame) output\n"
            try pipelines[0].terminal.feedOutput(Array(controlModeOutput.utf8))

        case .searchManyMatches:
            _ = try pipelines[0].terminal.search("target")
            try pipelines[0].terminal.feedOutput(Array("search refresh \(frame) target\n".utf8))
        }
    }

    /// p50/p95/p99 + mean of the samples (already in microseconds).
    static func percentiles(fromMicros samples: [Double]) -> BenchmarkBuildTimes {
        guard !samples.isEmpty else {
            return BenchmarkBuildTimes()
        }
        let sorted = samples.sorted()
        func percentile(_ fraction: Double) -> Double {
            let index = Int((Double(sorted.count - 1) * fraction).rounded())
            return sorted[min(max(0, index), sorted.count - 1)]
        }
        return BenchmarkBuildTimes(
            samples: sorted.count,
            meanMicros: samples.reduce(0, +) / Double(samples.count),
            p50Micros: percentile(0.5),
            p95Micros: percentile(0.95),
            p99Micros: percentile(0.99)
        )
    }

    private static func poll(
        _ pipeline: Pipeline,
        _ recorder: NativeRenderMetricsRecorder,
        forceFull: Bool
    ) throws -> Double? {
        _ = try pipeline.terminal.drainEvents()
        let update = try pipeline.terminal.frameUpdate(forceFull: forceFull)
        let result = pipeline.store.apply(update)
        recorder.recordFrameUpdate(update, applyResult: result)
        guard forceFull || result.changed else {
            recorder.recordSkippedPresent()
            return nil
        }
        let damage: TerminalDamage = forceFull ? .full : result.effectiveDamage
        let start = DispatchTime.now().uptimeNanoseconds
        pipeline.planCache.update(frame: pipeline.store.frame, renderConfig: .default, damage: damage)
        if let delay = ProcessInfo.processInfo.environment["TERMY_BENCHMARK_BUILD_DELAY_MICROS"].flatMap(UInt32.init),
           delay > 0
        {
            usleep(delay)
        }
        let micros = Double(DispatchTime.now().uptimeNanoseconds - start) / 1000.0
        recorder.recordPresentedFrame(planStats: pipeline.planCache.stats)
        return micros
    }
}
