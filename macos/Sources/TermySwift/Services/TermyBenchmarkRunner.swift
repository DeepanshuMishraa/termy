import Foundation

/// Render-plan-build time percentiles (microseconds) over a benchmark run. This
/// is the CPU render cost; full GPU frame-time gating still needs a windowed run
/// under xctrace.
struct BenchmarkBuildTimes: Codable, Equatable {
    var samples = 0
    var meanMicros = 0.0
    var p50Micros = 0.0
    var p95Micros = 0.0
    var p99Micros = 0.0

    var encodedJSON: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        guard let data = try? encoder.encode(self), let json = String(data: data, encoding: .utf8) else {
            return "{}"
        }
        return json
    }
}

struct BenchmarkResult {
    var metrics: NativeRenderMetricsSnapshot
    var buildTimes: BenchmarkBuildTimes
}

/// Headless render-pipeline benchmark. When the app is launched with
/// `--benchmark` it runs a deterministic workload, prints the native metrics +
/// render-plan-build percentiles as JSON, and exits before the GUI starts —
/// giving xtask's `benchmark-compare` a native target via `TERMY_BENCHMARK_COMMAND`.
enum TermyBenchmarkRunner {
    static let argument = "--benchmark"

    static func runIfRequested() {
        guard CommandLine.arguments.contains(argument) else {
            return
        }
        let result = run()
        print("native-render-metrics \(result.metrics.encodedJSON)")
        print("native-build-times \(result.buildTimes.encodedJSON)")
        exit(0)
    }

    /// Drives bulk scrolling output (a `cat` of a large file) through the real
    /// terminal → frame-store → render-plan pipeline and returns metrics plus
    /// per-present render-plan build times.
    static func run(cols: Int = 80, rows: Int = 24, frames: Int = 120) -> BenchmarkResult {
        let recorder = NativeRenderMetricsRecorder()
        guard let terminal = try? LibTermyTerminal(
            cols: UInt16(cols),
            rows: UInt16(rows),
            loadUserConfig: false
        ) else {
            return BenchmarkResult(metrics: recorder.snapshot, buildTimes: BenchmarkBuildTimes())
        }
        let store = TerminalFrameStore()
        let planCache = TerminalRenderPlanCache()
        var buildMicros: [Double] = []

        _ = try? poll(terminal, store, planCache, recorder, forceFull: true)

        let bulk = String(repeating: "native benchmark scrolling line of terminal text\n", count: 400)
        try? terminal.write(Array(bulk.utf8))

        for _ in 0..<frames {
            if let micros = try? poll(terminal, store, planCache, recorder, forceFull: false) {
                buildMicros.append(micros)
            }
            usleep(2_000)
        }
        return BenchmarkResult(metrics: recorder.snapshot, buildTimes: percentiles(fromMicros: buildMicros))
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

    /// Returns the render-plan build time in microseconds, or nil for a present
    /// that was skipped (no change).
    private static func poll(
        _ terminal: LibTermyTerminal,
        _ store: TerminalFrameStore,
        _ planCache: TerminalRenderPlanCache,
        _ recorder: NativeRenderMetricsRecorder,
        forceFull: Bool
    ) throws -> Double? {
        _ = try terminal.drainEvents()
        let update = try terminal.frameUpdate(forceFull: forceFull)
        let result = store.apply(update)
        recorder.recordFrameUpdate(update, applyResult: result)
        guard forceFull || result.changed else {
            recorder.recordSkippedPresent()
            return nil
        }
        let damage: TerminalDamage = forceFull ? .full : result.effectiveDamage
        let start = DispatchTime.now().uptimeNanoseconds
        planCache.update(frame: store.frame, renderConfig: .default, damage: damage)
        let micros = Double(DispatchTime.now().uptimeNanoseconds - start) / 1000.0
        recorder.recordPresentedFrame(planStats: planCache.stats)
        return micros
    }
}
