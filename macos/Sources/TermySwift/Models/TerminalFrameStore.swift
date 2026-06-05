import Foundation

struct TerminalFrameUpdate: Equatable {
    var cols: Int
    var rows: Int
    var cells: [TerminalCell]
    var cursor: TerminalCursor?
    var displayOffset: Int
    var historySize: Int
    var damage: TerminalDamage
}

struct TerminalFrameStoreApplyResult: Equatable {
    var changed: Bool
    var effectiveDamage: TerminalDamage
    var patchedCellCount: Int
}

final class TerminalFrameStore {
    private(set) var frame: TerminalFrame = .empty

    func reset(to frame: TerminalFrame) {
        self.frame = frame
    }

    @discardableResult
    func apply(_ update: TerminalFrameUpdate) -> TerminalFrameStoreApplyResult {
        if update.damage == .full || dimensionsChanged(for: update) || frame.cells.isEmpty {
            frame = TerminalFrame(
                cols: update.cols,
                rows: update.rows,
                cells: normalizedFullCells(from: update),
                cursor: update.cursor,
                displayOffset: update.displayOffset,
                historySize: update.historySize
            )
            return TerminalFrameStoreApplyResult(
                changed: true,
                effectiveDamage: .full,
                patchedCellCount: update.cells.count
            )
        }

        var nextFrame = frame
        nextFrame.cursor = update.cursor
        nextFrame.displayOffset = update.displayOffset
        nextFrame.historySize = update.historySize

        var patchedRows = Set<Int>()
        var patchedCellCount = 0
        for cell in update.cells {
            guard cell.row >= 0, cell.row < nextFrame.rows, cell.col >= 0, cell.col < nextFrame.cols else {
                continue
            }
            let index = (cell.row * nextFrame.cols) + cell.col
            guard nextFrame.cells.indices.contains(index) else {
                continue
            }
            nextFrame.cells[index] = cell
            patchedRows.insert(cell.row)
            patchedCellCount += 1
        }

        let metadataDamage = metadataDamage(from: frame, to: nextFrame)
        frame = nextFrame

        let effectiveDamage = mergedDamage(
            cellDamage: update.damage,
            patchedRows: patchedRows,
            metadataDamage: metadataDamage
        )
        return TerminalFrameStoreApplyResult(
            changed: effectiveDamage.hasChanges,
            effectiveDamage: effectiveDamage,
            patchedCellCount: patchedCellCount
        )
    }

    private func dimensionsChanged(for update: TerminalFrameUpdate) -> Bool {
        frame.cols != update.cols || frame.rows != update.rows
    }

    private func normalizedFullCells(from update: TerminalFrameUpdate) -> [TerminalCell] {
        let expectedCount = update.cols * update.rows
        guard expectedCount > 0 else {
            return []
        }
        if update.cells.count == expectedCount {
            return update.cells
        }

        var cells = (0..<update.rows).flatMap { row in
            (0..<update.cols).map { col in
                TerminalCell(
                    col: col,
                    row: row,
                    character: " ",
                    foreground: .termyForeground,
                    background: .termyBackground,
                    usesTerminalDefaultBackground: true,
                    renderText: false,
                    bold: false
                )
            }
        }
        for cell in update.cells {
            guard cell.row >= 0, cell.row < update.rows, cell.col >= 0, cell.col < update.cols else {
                continue
            }
            cells[(cell.row * update.cols) + cell.col] = cell
        }
        return cells
    }

    private func metadataDamage(from oldFrame: TerminalFrame, to newFrame: TerminalFrame) -> TerminalDamage {
        if oldFrame.displayOffset != newFrame.displayOffset || oldFrame.historySize != newFrame.historySize {
            return .full
        }

        var rows = Set<Int>()
        if oldFrame.cursor != newFrame.cursor {
            if let oldCursor = oldFrame.cursor, oldCursor.row >= 0, oldCursor.row < newFrame.rows {
                rows.insert(oldCursor.row)
            }
            if let newCursor = newFrame.cursor, newCursor.row >= 0, newCursor.row < newFrame.rows {
                rows.insert(newCursor.row)
            }
        }

        guard !rows.isEmpty else {
            return .none
        }
        return .partial(rows.sorted().map { row in
            TerminalDirtySpan(row: row, leftCol: 0, rightCol: max(0, newFrame.cols - 1))
        })
    }

    private func mergedDamage(
        cellDamage: TerminalDamage,
        patchedRows: Set<Int>,
        metadataDamage: TerminalDamage
    ) -> TerminalDamage {
        if cellDamage == .full || metadataDamage == .full {
            return .full
        }

        var spans: [TerminalDirtySpan] = []
        if case let .partial(cellSpans) = cellDamage {
            spans.append(contentsOf: cellSpans)
        }
        if case let .partial(metadataSpans) = metadataDamage {
            spans.append(contentsOf: metadataSpans)
        }
        for row in patchedRows {
            spans.append(TerminalDirtySpan(row: row, leftCol: 0, rightCol: max(0, frame.cols - 1)))
        }

        return TerminalDamage.partialCoalesced(spans)
    }
}
