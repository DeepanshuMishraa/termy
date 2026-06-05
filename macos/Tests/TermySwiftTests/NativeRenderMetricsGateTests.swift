import XCTest
@testable import TermySwift

final class NativeRenderMetricsGateTests: XCTestCase {
    func testPartialFrameUpdatesStayPartialAndMachineReadable() throws {
        let recorder = NativeRenderMetricsRecorder()
        let store = TerminalFrameStore()

        let fullUpdate = TerminalFrameUpdate(
            cols: 3,
            rows: 2,
            cells: [
                cell("a", col: 0, row: 0),
                cell("b", col: 1, row: 0),
                cell("c", col: 2, row: 0),
                cell("d", col: 0, row: 1),
                cell("e", col: 1, row: 1),
                cell("f", col: 2, row: 1)
            ],
            cursor: nil,
            displayOffset: 0,
            historySize: 0,
            damage: .full
        )
        let fullResult = store.apply(fullUpdate)
        recorder.recordFrameUpdate(fullUpdate, applyResult: fullResult)
        recorder.recordPresentedFrame(planStats: TerminalRenderPlanStats(
            wasFullRebuild: true,
            rebuiltRowCount: 2,
            totalRowCount: 2
        ))

        for index in 0..<5 {
            let update = TerminalFrameUpdate(
                cols: 3,
                rows: 2,
                cells: [cell("\(index)", col: 1, row: 0)],
                cursor: nil,
                displayOffset: 0,
                historySize: 0,
                damage: .partial([TerminalDirtySpan(row: 0, leftCol: 1, rightCol: 1)])
            )
            let result = store.apply(update)
            recorder.recordFrameUpdate(update, applyResult: result)
            recorder.recordPresentedFrame(planStats: TerminalRenderPlanStats(
                wasFullRebuild: false,
                rebuiltRowCount: 1,
                totalRowCount: 2
            ))
        }
        recorder.recordSkippedPresent()

        let metrics = recorder.snapshot
        XCTAssertEqual(metrics.frameUpdates, 6)
        XCTAssertEqual(metrics.fullFrameUpdates, 1)
        XCTAssertEqual(metrics.partialFrameUpdates, 5)
        XCTAssertEqual(metrics.presentedFrames, 6)
        XCTAssertEqual(metrics.skippedPresents, 1)
        XCTAssertEqual(metrics.fullRenderPlanRebuilds, 1)
        XCTAssertEqual(metrics.partialRenderPlanRebuilds, 5)
        XCTAssertEqual(metrics.patchedCells, 11)
        XCTAssertEqual(metrics.rebuiltRows, 7)

        let json = metrics.encodedJSON
        XCTAssertTrue(json.contains("\"partialFrameUpdates\":5"))
        XCTAssertTrue(json.contains("\"patchedCells\":11"))
    }

    func testRealTerminalWorkloadEmitsPartialNativeMetrics() throws {
        let terminal = try LibTermyTerminal(cols: 32, rows: 8, loadUserConfig: false)
        let store = TerminalFrameStore()
        let planCache = TerminalRenderPlanCache()
        let recorder = NativeRenderMetricsRecorder()

        try poll(
            terminal: terminal,
            store: store,
            planCache: planCache,
            recorder: recorder,
            forceFull: true
        )

        let command = "printf 'native-metrics-1\\nnative-metrics-2\\nnative-metrics-3\\n'\n"
        try terminal.write(Array(command.utf8))

        for _ in 0..<30 {
            try poll(
                terminal: terminal,
                store: store,
                planCache: planCache,
                recorder: recorder,
                forceFull: false
            )
            if recorder.snapshot.partialFrameUpdates > 0 {
                break
            }
            usleep(20_000)
        }

        let metrics = recorder.snapshot
        print("native-render-metrics \(metrics.encodedJSON)")
        XCTAssertEqual(metrics.fullFrameUpdates, 1)
        XCTAssertGreaterThan(metrics.partialFrameUpdates, 0)
        XCTAssertGreaterThan(metrics.presentedFrames, 1)
        XCTAssertGreaterThan(metrics.patchedCells, 32 * 8)
        XCTAssertLessThan(
            metrics.fullRenderPlanRebuilds,
            metrics.presentedFrames,
            "real workload should not force a full render-plan rebuild for every presented frame"
        )
    }
}

private func poll(
    terminal: LibTermyTerminal,
    store: TerminalFrameStore,
    planCache: TerminalRenderPlanCache,
    recorder: NativeRenderMetricsRecorder,
    forceFull: Bool
) throws {
    _ = try terminal.drainEvents()
    let update = try terminal.frameUpdate(forceFull: forceFull)
    let result = store.apply(update)
    recorder.recordFrameUpdate(update, applyResult: result)
    guard forceFull || result.changed else {
        recorder.recordSkippedPresent()
        return
    }
    let damage: TerminalDamage = forceFull ? .full : result.effectiveDamage
    planCache.update(
        frame: store.frame,
        renderConfig: .default,
        damage: damage
    )
    recorder.recordPresentedFrame(planStats: planCache.stats)
}

private func cell(_ character: String, col: Int, row: Int) -> TerminalCell {
    TerminalCell(
        col: col,
        row: row,
        character: Character(character),
        foreground: .termyForeground,
        background: .termyBackground,
        usesTerminalDefaultBackground: true,
        renderText: true,
        bold: false
    )
}
