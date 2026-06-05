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
            isFocused: isFocused
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
            isFocused: isFocused
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

    override var isFlipped: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.isOpaque = false
        canDrawSubviewsIntoLayer = true
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
        layer?.isOpaque = false
        canDrawSubviewsIntoLayer = true
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
        isFocused: Bool
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
        drawText(in: dirtyRect)
        drawHoveredLink(in: dirtyRect)
    }

    private func cellRect(col: Int, row: Int, cols: Int = 1) -> CGRect {
        CGRect(
            x: renderConfig.paddingX + CGFloat(col) * renderConfig.cellWidth,
            y: renderConfig.paddingY + CGFloat(row) * renderConfig.cellHeight,
            width: CGFloat(cols) * renderConfig.cellWidth,
            height: renderConfig.cellHeight
        ).pixelAligned(scale: window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1)
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
            let endCol = min(range.endCol, lastContentColumn(inRow: range.row))
            guard endCol >= range.startCol else {
                continue
            }
            fill(cellRect(
                col: range.startCol,
                row: range.row,
                cols: endCol - range.startCol + 1
            ))
        }
    }

    private func lastContentColumn(inRow row: Int) -> Int {
        var last = -1
        for cell in terminalFrame.cells(inRow: row) {
            let scalar = cell.character.unicodeScalars.first?.value ?? 0
            if scalar != 0, !cell.character.isWhitespace {
                last = cell.col
            }
        }
        return last
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
            return NSColor.systemOrange.withAlphaComponent(0.55)
        }
        return NSColor.systemYellow.withAlphaComponent(0.28)
    }

    private func drawCursor(in dirtyRect: NSRect) {
        guard isTerminalFocused,
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

    private func drawText(in dirtyRect: NSRect) {
        let regularFont = terminalFont(weight: .regular)
        let boldFont = terminalFont(weight: .semibold)
        let baselineOffset = max(0, (renderConfig.cellHeight - regularFont.ascender + regularFont.descender) / 2)
            + regularFont.ascender

        for segment in renderPlan.textSegments where rowIntersectsDirty(segment.row, dirtyRect) {
            let font = segment.bold ? boldFont : regularFont
            let point = CGPoint(
                x: renderConfig.paddingX + CGFloat(segment.startCol) * renderConfig.cellWidth,
                y: renderConfig.paddingY + CGFloat(segment.row) * renderConfig.cellHeight
                    + baselineOffset - font.ascender
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
    var nsColor: NSColor {
        NSColor(
            calibratedRed: red,
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
