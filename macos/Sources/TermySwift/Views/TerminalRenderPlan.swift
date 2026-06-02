import Foundation

/// A horizontal run of identically-styled background cells on one row.
struct TerminalBackgroundRun: Equatable {
    var row: Int
    var startCol: Int
    var cols: Int
    var color: TerminalRGBA
    var opacity: Double

    func canAppend(cell: TerminalCell, opacity: Double) -> Bool {
        cell.row == row
            && cell.col == startCol + cols
            && cell.background == color
            && opacity == self.opacity
    }
}

/// A run of glyphs that share a foreground color and weight on one row.
struct TerminalTextSegment: Equatable {
    var row: Int
    var startCol: Int
    var text: String
    var foreground: TerminalRGBA
    var bold: Bool
}

/// The flattened paint instructions for a whole frame: the grid view draws these
/// directly without re-deriving them from cells.
struct TerminalRenderPlan: Equatable {
    var backgroundRuns: [TerminalBackgroundRun]
    var textSegments: [TerminalTextSegment]

    static let empty = TerminalRenderPlan(backgroundRuns: [], textSegments: [])
}

/// The paint instructions for a single grid row, cached so unchanged rows are
/// reused across frames.
private struct RowRenderPlan: Equatable {
    var backgroundRuns: [TerminalBackgroundRun]
    var textSegments: [TerminalTextSegment]
}

/// How the most recent `update` rebuilt the plan — surfaced to the debug overlay
/// to validate that small changes only rebuild a few rows.
struct TerminalRenderPlanStats: Equatable {
    var wasFullRebuild: Bool
    var rebuiltRowCount: Int
    var totalRowCount: Int

    static let empty = TerminalRenderPlanStats(
        wasFullRebuild: false,
        rebuiltRowCount: 0,
        totalRowCount: 0
    )
}

/// Builds and caches per-row render plans, rebuilding only the rows the terminal
/// core flagged as damaged. A full rebuild happens when the render config or grid
/// dimensions change, or when the core reports full damage; otherwise only the
/// damaged rows are re-laid-out and the rest are reused.
final class TerminalRenderPlanCache {
    private(set) var plan = TerminalRenderPlan.empty
    private(set) var stats = TerminalRenderPlanStats.empty

    private var rows: [RowRenderPlan] = []
    private var cachedConfig: TerminalRenderConfig?
    private var cachedCols = 0
    private var cachedRows = 0

    /// Updates the cached plan for `frame`, rebuilding rows according to `damage`.
    func update(frame: TerminalFrame, renderConfig: TerminalRenderConfig, damage: TerminalDamage) {
        let configChanged = cachedConfig != renderConfig
        let dimensionsChanged = cachedCols != frame.cols
            || cachedRows != frame.rows
            || rows.count != frame.rows
        let forceFull = configChanged || dimensionsChanged || damage == .full

        if forceFull {
            rows = (0..<frame.rows).map { row in
                buildRow(row, frame: frame, renderConfig: renderConfig)
            }
            stats = TerminalRenderPlanStats(
                wasFullRebuild: true,
                rebuiltRowCount: frame.rows,
                totalRowCount: frame.rows
            )
        } else {
            let dirtyRows = dirtyRows(for: damage, rowCount: frame.rows)
            for row in dirtyRows {
                rows[row] = buildRow(row, frame: frame, renderConfig: renderConfig)
            }
            stats = TerminalRenderPlanStats(
                wasFullRebuild: false,
                rebuiltRowCount: dirtyRows.count,
                totalRowCount: frame.rows
            )
        }

        cachedConfig = renderConfig
        cachedCols = frame.cols
        cachedRows = frame.rows
        flatten()
    }

    private func dirtyRows(for damage: TerminalDamage, rowCount: Int) -> [Int] {
        guard case let .partial(spans) = damage else {
            return []
        }
        var seen = Set<Int>()
        var ordered: [Int] = []
        for span in spans where span.row >= 0 && span.row < rowCount {
            if seen.insert(span.row).inserted {
                ordered.append(span.row)
            }
        }
        return ordered
    }

    private func flatten() {
        var backgroundRuns: [TerminalBackgroundRun] = []
        var textSegments: [TerminalTextSegment] = []
        for row in rows {
            backgroundRuns.append(contentsOf: row.backgroundRuns)
            textSegments.append(contentsOf: row.textSegments)
        }
        plan = TerminalRenderPlan(backgroundRuns: backgroundRuns, textSegments: textSegments)
    }

    private func buildRow(
        _ row: Int,
        frame: TerminalFrame,
        renderConfig: TerminalRenderConfig
    ) -> RowRenderPlan {
        var backgroundRuns: [TerminalBackgroundRun] = []
        var textSegments: [TerminalTextSegment] = []

        var activeBackgroundRun: TerminalBackgroundRun?
        var text = ""
        var textForeground: TerminalRGBA?
        var textBold = false
        var textStartCol = 0

        func flushBackgroundRun() {
            guard let run = activeBackgroundRun else {
                return
            }
            backgroundRuns.append(run)
            activeBackgroundRun = nil
        }

        func flushTextSegment() {
            guard let foreground = textForeground, !text.isEmpty else {
                return
            }
            textSegments.append(TerminalTextSegment(
                row: row,
                startCol: textStartCol,
                text: text,
                foreground: foreground,
                bold: textBold
            ))
            text = ""
        }

        for cell in frame.cells(inRow: row) {
            if shouldPaintBackground(cell, renderConfig: renderConfig) {
                let opacity = backgroundOpacity(for: cell, renderConfig: renderConfig)
                if var run = activeBackgroundRun, run.canAppend(cell: cell, opacity: opacity) {
                    run.cols += 1
                    activeBackgroundRun = run
                } else {
                    flushBackgroundRun()
                    activeBackgroundRun = TerminalBackgroundRun(
                        row: row,
                        startCol: cell.col,
                        cols: 1,
                        color: cell.background,
                        opacity: opacity
                    )
                }
            } else {
                flushBackgroundRun()
            }

            guard cell.renderText else {
                flushTextSegment()
                textForeground = nil
                textBold = false
                continue
            }

            let foreground = cell.foreground
            let bold = cell.bold
            if textForeground != foreground || textBold != bold {
                flushTextSegment()
                textForeground = foreground
                textBold = bold
                textStartCol = cell.col
            }
            text.append(cell.character)
        }

        flushBackgroundRun()
        flushTextSegment()

        return RowRenderPlan(backgroundRuns: backgroundRuns, textSegments: textSegments)
    }

    private func shouldPaintBackground(
        _ cell: TerminalCell,
        renderConfig: TerminalRenderConfig
    ) -> Bool {
        !cell.usesTerminalDefaultBackground || renderConfig.backgroundOpacityCells
    }

    private func backgroundOpacity(
        for cell: TerminalCell,
        renderConfig: TerminalRenderConfig
    ) -> Double {
        cell.usesTerminalDefaultBackground ? renderConfig.backgroundOpacity : 1.0
    }
}
