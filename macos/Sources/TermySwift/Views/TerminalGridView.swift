import AppKit
import SwiftUI

struct TerminalGridView: View {
    let frame: TerminalFrame
    let renderPlan: TerminalRenderPlan
    let renderDamage: TerminalDamage
    let selection: TerminalSelection?
    let renderConfig: TerminalRenderConfig
    let searchMatches: [TerminalSearchMatch]
    let activeSearchMatch: TerminalSearchMatch?
    var hoveredLink: TerminalFrameLink?
    let isFocused: Bool
    let isCursorVisible: Bool

    var body: some View {
        TerminalGridRepresentable(
            frame: frame,
            renderPlan: renderPlan,
            renderDamage: renderDamage,
            selection: selection,
            renderConfig: renderConfig,
            searchMatches: searchMatches,
            activeSearchMatch: activeSearchMatch,
            hoveredLink: hoveredLink,
            isFocused: isFocused,
            isCursorVisible: isCursorVisible
        )
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .clipped()
    }
}

private struct TerminalGridRepresentable: NSViewRepresentable {
    let frame: TerminalFrame
    let renderPlan: TerminalRenderPlan
    let renderDamage: TerminalDamage
    let selection: TerminalSelection?
    let renderConfig: TerminalRenderConfig
    let searchMatches: [TerminalSearchMatch]
    let activeSearchMatch: TerminalSearchMatch?
    var hoveredLink: TerminalFrameLink?
    let isFocused: Bool
    let isCursorVisible: Bool

    func makeNSView(context: Context) -> TerminalGridNSView {
        TerminalGridNSView()
    }

    func updateNSView(_ nsView: TerminalGridNSView, context: Context) {
        nsView.update(
            frame: frame,
            renderPlan: renderPlan,
            renderDamage: renderDamage,
            selection: selection,
            renderConfig: renderConfig,
            searchMatches: searchMatches,
            activeSearchMatch: activeSearchMatch,
            hoveredLink: hoveredLink,
            isFocused: isFocused,
            isCursorVisible: isCursorVisible
        )
    }
}

private final class TerminalGridNSView: NSView {
    private var terminalFrame = TerminalFrame.empty
    private var renderPlan = TerminalRenderPlan.empty
    private var selection: TerminalSelection?
    private var renderConfig = TerminalRenderConfig.default
    private var searchMatches: [TerminalSearchMatch] = []
    private var activeSearchMatch: TerminalSearchMatch?
    private var hoveredLink: TerminalFrameLink?
    private var isTerminalFocused = false
    private var isCursorVisible = true

    override var isFlipped: Bool { true }

    // Expose the visible grid as a text area so VoiceOver can read terminal
    // output; the value is recomputed live from the current frame.
    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .textArea }
    override func accessibilityLabel() -> String? { "Terminal" }
    override func accessibilityValue() -> Any? { terminalFrame.visibleTextSnapshot() }
    override func accessibilitySelectedText() -> String? { terminalFrame.selectedText(for: selection) }
    override func accessibilityInsertionPointLineNumber() -> Int { terminalFrame.cursor?.row ?? 0 }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        configureLayer()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        configureLayer()
    }

    private func configureLayer() {
        wantsLayer = true
        layer?.isOpaque = false
        canDrawSubviewsIntoLayer = true
        // Live resize: redraw the grid on every resize step and keep stale
        // content anchored to the top-left while the next draw is pending,
        // instead of letting Core Animation stretch the previous frame across
        // the new bounds (the rubber-band look).
        layerContentsRedrawPolicy = .duringViewResize
        layerContentsPlacement = .topLeft
    }

    func update(
        frame: TerminalFrame,
        renderPlan: TerminalRenderPlan,
        renderDamage: TerminalDamage,
        selection: TerminalSelection?,
        renderConfig: TerminalRenderConfig,
        searchMatches: [TerminalSearchMatch],
        activeSearchMatch: TerminalSearchMatch?,
        hoveredLink: TerminalFrameLink?,
        isFocused: Bool,
        isCursorVisible: Bool
    ) {
        let requiresFullRedraw = terminalFrame.cols != frame.cols
            || terminalFrame.rows != frame.rows
            || terminalFrame.displayOffset != frame.displayOffset
            || self.renderConfig != renderConfig
            || self.selection != selection
            || self.searchMatches != searchMatches
            || self.activeSearchMatch != activeSearchMatch
            || self.hoveredLink != hoveredLink
            || isTerminalFocused != isFocused

        self.terminalFrame = frame
        self.renderPlan = renderPlan
        self.selection = selection
        self.renderConfig = renderConfig
        self.searchMatches = searchMatches
        self.activeSearchMatch = activeSearchMatch
        self.hoveredLink = hoveredLink
        self.isTerminalFocused = isFocused
        self.isCursorVisible = isCursorVisible

        if requiresFullRedraw || renderDamage == .full {
            needsDisplay = true
            return
        }

        guard let dirtyRows = renderDamage.dirtyRows, !dirtyRows.isEmpty else {
            return
        }
        for row in dirtyRows {
            setNeedsDisplay(rowRect(row).insetBy(dx: -1, dy: -1))
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard terminalFrame.cols > 0, terminalFrame.rows > 0 else {
            return
        }

        drawBackgrounds(in: dirtyRect)
        drawSearch(in: dirtyRect)
        drawSelection(in: dirtyRect)
        drawCursor(in: dirtyRect)
        drawBlockGlyphs(in: dirtyRect)
        drawStrokeGlyphs(in: dirtyRect)
        drawText(in: dirtyRect)
        drawHoveredLink(in: dirtyRect)
    }

    private var backingScale: CGFloat {
        max(1, window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1)
    }

    private func cellRect(col: Int, row: Int, cols: Int = 1) -> CGRect {
        CGRect(
            x: renderConfig.paddingX + CGFloat(col) * renderConfig.cellWidth,
            y: renderConfig.paddingY + CGFloat(row) * renderConfig.cellHeight,
            width: CGFloat(cols) * renderConfig.cellWidth,
            height: renderConfig.cellHeight
        ).pixelAligned(scale: backingScale)
    }

    private func rowRect(_ row: Int) -> CGRect {
        CGRect(
            x: 0,
            y: renderConfig.paddingY + CGFloat(row) * renderConfig.cellHeight,
            width: bounds.width,
            height: renderConfig.cellHeight
        )
    }

    private func dirtyRowRange(for dirtyRect: NSRect) -> ClosedRange<Int> {
        let cellHeight = max(1, renderConfig.cellHeight)
        let first = max(0, Int(floor((dirtyRect.minY - renderConfig.paddingY) / cellHeight)))
        let last = min(
            max(0, terminalFrame.rows - 1),
            Int(ceil((dirtyRect.maxY - renderConfig.paddingY) / cellHeight))
        )
        return first...max(first, last)
    }

    private func rowIntersectsDirty(_ row: Int, _ dirtyRect: NSRect) -> Bool {
        dirtyRowRange(for: dirtyRect).contains(row)
    }

    private func drawBackgrounds(in dirtyRect: NSRect) {
        for run in renderPlan.backgroundRuns where rowIntersectsDirty(run.row, dirtyRect) {
            renderConfig.nsColor(run.color, opacity: run.opacity).setFill()
            fill(cellRect(col: run.startCol, row: run.row, cols: run.cols))
        }
    }

    private func drawSelection(in dirtyRect: NSRect) {
        guard let ranges = selection?.rowRanges(cols: terminalFrame.cols, rows: terminalFrame.rows) else {
            return
        }
        NSColor.controlAccentColor.withAlphaComponent(0.35).setFill()
        for range in ranges where rowIntersectsDirty(range.row, dirtyRect) {
            guard range.endCol >= range.startCol else {
                continue
            }
            fill(cellRect(
                col: range.startCol,
                row: range.row,
                cols: range.endCol - range.startCol + 1
            ))
        }
    }

    private func drawSearch(in dirtyRect: NSRect) {
        for match in searchMatches {
            guard let row = visibleSearchRow(for: match), rowIntersectsDirty(row, dirtyRect) else {
                continue
            }
            searchColor(for: match).setFill()
            fill(cellRect(
                col: match.startCol,
                row: row,
                cols: max(1, match.endCol - match.startCol + 1)
            ))
        }
    }

    private func visibleSearchRow(for match: TerminalSearchMatch) -> Int? {
        let visibleTop = terminalFrame.historySize - terminalFrame.displayOffset
        let row = match.row - visibleTop
        guard row >= 0, row < terminalFrame.rows else {
            return nil
        }
        return row
    }

    private func searchColor(for match: TerminalSearchMatch) -> NSColor {
        if match == activeSearchMatch {
            return NSColor.controlAccentColor.withAlphaComponent(0.42)
        }
        return NSColor.controlAccentColor.withAlphaComponent(0.18)
    }

    private func drawCursor(in dirtyRect: NSRect) {
        guard isTerminalFocused,
              isCursorVisible,
              let cursor = terminalFrame.cursor,
              terminalFrame.displayOffset == 0,
              rowIntersectsDirty(cursor.row, dirtyRect)
        else {
            return
        }

        let rect = cellRect(col: cursor.col, row: cursor.row)
        renderConfig.cursor.nsColor.setFill()
        switch cursor.style {
        case .block:
            fill(rect)
        case .line:
            fill(CGRect(x: rect.minX, y: rect.minY, width: min(2, rect.width), height: rect.height))
        }
    }

    /// Draws block-element and box-drawing cells as solid rects with every edge
    /// snapped to device pixels (see `TerminalBlockGlyphs.snappedRect` for the
    /// seam-free shared-boundary guarantee). Anti-aliasing is disabled so the
    /// axis-aligned fills keep hard edges, unlike font glyphs which only cover
    /// the font box inside the taller line-height cell.
    private func drawBlockGlyphs(in dirtyRect: NSRect) {
        let glyphs = renderPlan.blockGlyphs
        guard !glyphs.isEmpty else {
            return
        }

        let context = NSGraphicsContext.current
        let previousShouldAntialias = context?.shouldAntialias ?? true
        context?.shouldAntialias = false
        defer { context?.shouldAntialias = previousShouldAntialias }

        for glyph in glyphs where rowIntersectsDirty(glyph.row, dirtyRect) {
            for rect in glyph.rects {
                guard let snapped = TerminalBlockGlyphs.snappedRect(
                    rect,
                    col: glyph.col,
                    row: glyph.row,
                    cellWidth: renderConfig.cellWidth,
                    cellHeight: renderConfig.cellHeight,
                    paddingX: renderConfig.paddingX,
                    paddingY: renderConfig.paddingY,
                    scale: backingScale
                ) else {
                    continue
                }
                glyph.foreground.nsColor
                    .withAlphaComponent(glyph.foreground.alpha * rect.alpha)
                    .setFill()
                fill(snapped)
            }
        }
    }

    /// Strokes rounded-corner (U+256D–U+2570) and diagonal (U+2571–U+2573)
    /// box-drawing glyphs as paths whose stroke width matches the rect-drawn
    /// straight segments, mirroring `paint_rounded_corner_path` /
    /// `paint_diagonal_path` in `crates/terminal_ui/src/grid.rs`. These stay
    /// anti-aliased — they are curves and slopes, not axis-aligned fills.
    private func drawStrokeGlyphs(in dirtyRect: NSRect) {
        let glyphs = renderPlan.strokeGlyphs
        guard !glyphs.isEmpty else {
            return
        }

        let strokeWidth = TerminalBoxDrawing.strokeWidth(fontSize: renderConfig.fontSize)
        for glyph in glyphs where rowIntersectsDirty(glyph.row, dirtyRect) {
            guard let cell = TerminalBlockGlyphs.snappedRect(
                TerminalBlockRect(left: 0, top: 0, right: 1, bottom: 1, alpha: 1),
                col: glyph.col,
                row: glyph.row,
                cellWidth: renderConfig.cellWidth,
                cellHeight: renderConfig.cellHeight,
                paddingX: renderConfig.paddingX,
                paddingY: renderConfig.paddingY,
                scale: backingScale
            ) else {
                continue
            }

            glyph.foreground.nsColor.setStroke()
            switch glyph.kind {
            case .roundedCorner:
                strokeRoundedCorner(glyph.character, in: cell, strokeWidth: strokeWidth)
            case .diagonal:
                strokeDiagonal(glyph.character, in: cell, strokeWidth: strokeWidth)
            }
        }
    }

    /// Rounded corners: a short straight stub on each cell edge (aligned with
    /// adjacent straight box lines) connected by a quarter-circle cubic arc.
    private func strokeRoundedCorner(_ glyph: Character, in cell: CGRect, strokeWidth: CGFloat) {
        let radius = max(min(cell.width, cell.height) - strokeWidth, 0) / 2
        let ctrlOffset = radius / 4
        let centerX = snappedStrokeCenter(origin: cell.minX, size: cell.width, strokeWidth: strokeWidth)
        let centerY = snappedStrokeCenter(origin: cell.minY, size: cell.height, strokeWidth: strokeWidth)
        let leftCenter = CGPoint(x: cell.minX, y: centerY)
        let rightCenter = CGPoint(x: cell.maxX, y: centerY)
        let topCenter = CGPoint(x: centerX, y: cell.minY)
        let bottomCenter = CGPoint(x: centerX, y: cell.maxY)

        let spec: (start: CGPoint, curveStart: CGPoint, controlA: CGPoint, controlB: CGPoint, curveEnd: CGPoint, end: CGPoint)
        switch glyph {
        case "\u{256D}": // ╭
            spec = (
                bottomCenter,
                CGPoint(x: centerX, y: centerY + radius),
                CGPoint(x: centerX, y: centerY + ctrlOffset),
                CGPoint(x: centerX + ctrlOffset, y: centerY),
                CGPoint(x: centerX + radius, y: centerY),
                rightCenter
            )
        case "\u{256E}": // ╮
            spec = (
                bottomCenter,
                CGPoint(x: centerX, y: centerY + radius),
                CGPoint(x: centerX, y: centerY + ctrlOffset),
                CGPoint(x: centerX - ctrlOffset, y: centerY),
                CGPoint(x: centerX - radius, y: centerY),
                leftCenter
            )
        case "\u{256F}": // ╯
            spec = (
                topCenter,
                CGPoint(x: centerX, y: centerY - radius),
                CGPoint(x: centerX, y: centerY - ctrlOffset),
                CGPoint(x: centerX - ctrlOffset, y: centerY),
                CGPoint(x: centerX - radius, y: centerY),
                leftCenter
            )
        case "\u{2570}": // ╰
            spec = (
                topCenter,
                CGPoint(x: centerX, y: centerY - radius),
                CGPoint(x: centerX, y: centerY - ctrlOffset),
                CGPoint(x: centerX + ctrlOffset, y: centerY),
                CGPoint(x: centerX + radius, y: centerY),
                rightCenter
            )
        default:
            return
        }

        let path = NSBezierPath()
        path.move(to: spec.start)
        path.line(to: spec.curveStart)
        path.curve(to: spec.curveEnd, controlPoint1: spec.controlA, controlPoint2: spec.controlB)
        path.line(to: spec.end)
        path.lineWidth = strokeWidth
        path.stroke()
    }

    /// Diagonals: stroked straight lines whose endpoints overshoot the cell
    /// boundary by a slope-dependent amount so adjacent diagonal cells join
    /// without pixel gaps.
    private func strokeDiagonal(_ glyph: Character, in cell: CGRect, strokeWidth: CGFloat) {
        guard cell.width > 0, cell.height > 0 else {
            return
        }
        let slopeX = 0.5 * min(cell.width / cell.height, 1)
        let slopeY = 0.5 * min(cell.height / cell.width, 1)

        let upperRightToLowerLeft = (
            CGPoint(x: cell.maxX + slopeX, y: cell.minY - slopeY),
            CGPoint(x: cell.minX - slopeX, y: cell.maxY + slopeY)
        )
        let upperLeftToLowerRight = (
            CGPoint(x: cell.minX - slopeX, y: cell.minY - slopeY),
            CGPoint(x: cell.maxX + slopeX, y: cell.maxY + slopeY)
        )

        let lines: [(CGPoint, CGPoint)]
        switch glyph {
        case "\u{2571}": lines = [upperRightToLowerLeft] // ╱
        case "\u{2572}": lines = [upperLeftToLowerRight] // ╲
        case "\u{2573}": lines = [upperRightToLowerLeft, upperLeftToLowerRight] // ╳
        default: return
        }

        for (start, end) in lines {
            let path = NSBezierPath()
            path.move(to: start)
            path.line(to: end)
            path.lineWidth = strokeWidth
            path.stroke()
        }
    }

    /// Midpoint of a stroke pixel-snapped to integer boundaries: rounds both
    /// stroke edges independently, then averages. Prevents sub-pixel shimmer on
    /// odd-width strokes across HiDPI scales.
    private func snappedStrokeCenter(origin: CGFloat, size: CGFloat, strokeWidth: CGFloat) -> CGFloat {
        let center = origin + size / 2
        let minEdge = (center - strokeWidth / 2).rounded()
        let maxEdge = (center + strokeWidth / 2).rounded()
        return (minEdge + maxEdge) / 2
    }

    private func drawText(in dirtyRect: NSRect) {
        let regularFont = terminalFont(weight: .regular)
        let boldFont = terminalFont(weight: .semibold)
        let baselineOffset = max(0, (renderConfig.cellHeight - regularFont.ascender + regularFont.descender) / 2)
            + regularFont.ascender

        let scale = backingScale
        for segment in renderPlan.textSegments where rowIntersectsDirty(segment.row, dirtyRect) {
            let font = segment.bold ? boldFont : regularFont
            // Snap the glyph origin to device pixels so baselines land on
            // consistent pixel rows, matching the pixel-aligned backgrounds.
            let point = CGPoint(
                x: ((renderConfig.paddingX + CGFloat(segment.startCol) * renderConfig.cellWidth)
                    * scale).rounded() / scale,
                y: ((renderConfig.paddingY + CGFloat(segment.row) * renderConfig.cellHeight
                    + baselineOffset - font.ascender) * scale).rounded() / scale
            )
            (segment.text as NSString).draw(
                at: point,
                withAttributes: [
                    .font: font,
                    .foregroundColor: segment.foreground.nsColor
                ]
            )
        }
    }

    private func terminalFont(weight: NSFont.Weight) -> NSFont {
        let family = renderConfig.fontFamily.trimmingCharacters(in: .whitespacesAndNewlines)
        if !family.isEmpty, let font = NSFont(name: family, size: renderConfig.fontSize) {
            return weight == .semibold ? NSFontManager.shared.convertWeight(true, of: font) : font
        }
        return NSFont.monospacedSystemFont(ofSize: renderConfig.fontSize, weight: weight)
    }

    private func drawHoveredLink(in dirtyRect: NSRect) {
        guard let link = hoveredLink,
              link.row >= 0,
              link.row < terminalFrame.rows,
              rowIntersectsDirty(link.row, dirtyRect)
        else {
            return
        }

        let rect = cellRect(
            col: link.startCol,
            row: link.row,
            cols: max(1, link.endCol - link.startCol + 1)
        )
        let path = NSBezierPath()
        path.move(to: CGPoint(x: rect.minX, y: rect.maxY - 1))
        path.line(to: CGPoint(x: rect.maxX, y: rect.maxY - 1))
        path.lineWidth = 1
        renderConfig.foreground.nsColor.withAlphaComponent(0.85).setStroke()
        path.stroke()
    }

    private func fill(_ rect: CGRect) {
        NSBezierPath(rect: rect).fill()
    }
}

private extension TerminalRGBA {
    /// Terminal escape-sequence colors are sRGB by convention; the calibrated
    /// (generic) color space renders them visibly duller, and the settings UI
    /// already builds its swatches in sRGB.
    var nsColor: NSColor {
        NSColor(
            srgbRed: red,
            green: green,
            blue: blue,
            alpha: alpha
        )
    }
}

private extension TerminalRenderConfig {
    func nsColor(_ color: TerminalRGBA, opacity: Double = 1.0) -> NSColor {
        color.nsColor.withAlphaComponent(color.alpha * opacity)
    }
}

private extension CGRect {
    func pixelAligned(scale: CGFloat) -> CGRect {
        let scale = max(1, scale)
        let minX = floor(self.minX * scale) / scale
        let minY = floor(self.minY * scale) / scale
        let maxX = ceil(self.maxX * scale) / scale
        let maxY = ceil(self.maxY * scale) / scale
        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
    }
}
