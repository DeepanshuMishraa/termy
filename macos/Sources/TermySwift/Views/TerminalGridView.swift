import SwiftUI

struct TerminalGridView: View {
    @Environment(\.displayScale) private var displayScale

    let frame: TerminalFrame
    let renderPlan: TerminalRenderPlan
    let selection: TerminalSelection?
    let renderConfig: TerminalRenderConfig
    let searchMatches: [TerminalSearchMatch]
    let activeSearchMatch: TerminalSearchMatch?
    var hoveredLink: TerminalFrameLink?
    let isFocused: Bool

    var body: some View {
        Canvas(opaque: false, rendersAsynchronously: false) { context, _ in
            draw(in: &context)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .clipped()
    }

    private func draw(in context: inout GraphicsContext) {
        drawBackgrounds(renderPlan.backgroundRuns, in: &context)
        drawSearch(in: &context)
        drawSelection(in: &context)
        drawCursor(in: &context)
        drawText(renderPlan.textSegments, in: &context)
        drawHoveredLink(in: &context)
    }

    private func drawHoveredLink(in context: inout GraphicsContext) {
        guard let link = hoveredLink, link.row >= 0, link.row < frame.rows else {
            return
        }
        let rect = cellRect(
            col: link.startCol,
            row: link.row,
            cols: max(1, link.endCol - link.startCol + 1)
        )
        let y = rect.maxY - 1
        var path = Path()
        path.move(to: CGPoint(x: rect.minX, y: y))
        path.addLine(to: CGPoint(x: rect.maxX, y: y))
        context.stroke(
            path,
            with: .color(renderConfig.foreground.swiftUIColor.opacity(0.85)),
            lineWidth: 1
        )
    }

    private func cellRect(col: Int, row: Int, cols: Int = 1) -> CGRect {
        CGRect(
            x: renderConfig.paddingX + CGFloat(col) * renderConfig.cellWidth,
            y: renderConfig.paddingY + CGFloat(row) * renderConfig.cellHeight,
            width: CGFloat(cols) * renderConfig.cellWidth,
            height: renderConfig.cellHeight
        )
    }

    private func pixelAlignedCellRect(col: Int, row: Int, cols: Int = 1) -> CGRect {
        pixelAligned(cellRect(col: col, row: row, cols: cols))
    }

    private func pixelAligned(_ rect: CGRect) -> CGRect {
        let scale = max(1.0, displayScale)
        let minX = floor(rect.minX * scale) / scale
        let minY = floor(rect.minY * scale) / scale
        let maxX = ceil(rect.maxX * scale) / scale
        let maxY = ceil(rect.maxY * scale) / scale
        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }

    private func drawBackgrounds(_ runs: [TerminalBackgroundRun], in context: inout GraphicsContext) {
        for run in runs {
            let rect = pixelAlignedCellRect(col: run.startCol, row: run.row, cols: run.cols)
            context.fill(
                Path(rect),
                with: .color(run.color.swiftUIColor.opacity(run.opacity))
            )
        }
    }

    private func drawSelection(in context: inout GraphicsContext) {
        guard let ranges = selection?.rowRanges(cols: frame.cols, rows: frame.rows) else {
            return
        }
        let fill = GraphicsContext.Shading.color(Color.accentColor.opacity(0.35))
        for range in ranges {
            let rect = pixelAlignedCellRect(
                col: range.startCol,
                row: range.row,
                cols: range.endCol - range.startCol + 1
            )
            context.fill(Path(rect), with: fill)
        }
    }

    private func drawSearch(in context: inout GraphicsContext) {
        for match in searchMatches {
            guard let row = visibleSearchRow(for: match) else {
                continue
            }
            let rect = pixelAlignedCellRect(
                col: match.startCol,
                row: row,
                cols: max(1, match.endCol - match.startCol + 1)
            )
            context.fill(Path(rect), with: .color(searchColor(for: match)))
        }
    }

    private func visibleSearchRow(for match: TerminalSearchMatch) -> Int? {
        let visibleTop = frame.historySize - frame.displayOffset
        let row = match.row - visibleTop
        guard row >= 0, row < frame.rows else {
            return nil
        }
        return row
    }

    private func searchColor(for match: TerminalSearchMatch) -> Color {
        if match == activeSearchMatch {
            return .orange.opacity(0.55)
        }
        return .yellow.opacity(0.28)
    }

    private func drawCursor(in context: inout GraphicsContext) {
        guard isFocused, let cursor = frame.cursor, frame.displayOffset == 0 else {
            return
        }
        let rect = pixelAlignedCellRect(col: cursor.col, row: cursor.row)
        context.fill(Path(rect), with: .color(renderConfig.cursor.swiftUIColor))
    }

    private func drawText(_ segments: [TerminalTextSegment], in context: inout GraphicsContext) {
        let centerY = renderConfig.cellHeight / 2
        for segment in segments {
            var text = Text(verbatim: segment.text)
                .font(terminalFont(weight: segment.bold ? .semibold : .regular))
            text = text.foregroundColor(segment.foreground.swiftUIColor)

            let resolved = context.resolve(text)
            let origin = CGPoint(
                x: renderConfig.paddingX + CGFloat(segment.startCol) * renderConfig.cellWidth,
                y: renderConfig.paddingY + CGFloat(segment.row) * renderConfig.cellHeight + centerY
            )
            context.draw(resolved, at: origin, anchor: .leading)
        }
    }

    private func terminalFont(weight: Font.Weight) -> Font {
        let fontFamily = renderConfig.fontFamily.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !fontFamily.isEmpty else {
            return .system(size: renderConfig.fontSize, weight: weight, design: .monospaced)
        }
        return .custom(fontFamily, size: renderConfig.fontSize).weight(weight)
    }
}
