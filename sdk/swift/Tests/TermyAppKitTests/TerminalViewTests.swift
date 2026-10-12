import AppKit
import Testing
import Termy
@testable import TermyAppKit

@Suite("Terminal view")
@MainActor
struct TerminalViewTests {
    private func makeView(feeding output: String = "") throws -> TerminalView {
        let config = try TermyConfig(contents: "padding_x = 4\npadding_y = 4\ncursor_blink = false\n")
        let render = try config.renderConfig(for: .dark)
        let session = try TerminalSession.display(
            size: TerminalSize(cols: 20, rows: 5, cellWidth: render.cellWidth, cellHeight: render.cellHeight))
        let view = TerminalView(session: session, renderConfig: render)
        view.frame = NSRect(origin: .zero, size: view.size(cols: 20, rows: 5))
        if !output.isEmpty { try session.feed(Data(output.utf8)) }
        return view
    }

    private func render(_ view: TerminalView) throws -> NSBitmapImageRep {
        let bitmap = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: bitmap)
        return bitmap
    }

    private func color(in bitmap: NSBitmapImageRep, view: TerminalView, row: Int, col: Int) throws -> NSColor {
        let rect = view.renderer.cellRect(row: row, col: col)
        let scale = CGFloat(bitmap.pixelsWide) / view.bounds.width
        let color = try #require(bitmap.colorAt(x: Int(rect.midX * scale), y: Int(rect.midY * scale)))
        return try #require(color.usingColorSpace(.sRGB))
    }

    @Test func sizesTheGridToTheView() throws {
        let view = try makeView()
        #expect(view.session.size.cols == 20)
        #expect(view.session.size.rows == 5)
        view.setFrameSize(view.size(cols: 30, rows: 8))
        #expect(view.session.size.cols == 30)
        #expect(view.session.size.rows == 8)
        #expect(view.session.frame.cols == 30)
    }

    @Test func rendersCellBackgrounds() throws {
        let view = try makeView(feeding: "\u{1b}[41m  \u{1b}[0m")
        let bitmap = try render(view)
        let red = try color(in: bitmap, view: view, row: 0, col: 0)
        #expect(red.redComponent > 0.5)
        #expect(red.greenComponent < 0.4)

        let background = try color(in: bitmap, view: view, row: 3, col: 10)
        let expected = view.renderConfig.background
        #expect(abs(background.redComponent - CGFloat(expected.r) / 255) < 0.05)
        #expect(abs(background.blueComponent - CGFloat(expected.b) / 255) < 0.05)
    }

    @Test func rendersGlyphsInsideTheirCells() throws {
        let view = try makeView(feeding: "\u{1b}[37mHH")
        let bitmap = try render(view)
        let rect = view.renderer.cellRect(row: 0, col: 0, width: 2)
        let scale = CGFloat(bitmap.pixelsWide) / view.bounds.width
        var brightest: CGFloat = 0
        for y in Int(rect.minY * scale)..<Int(rect.maxY * scale) {
            for x in Int(rect.minX * scale)..<Int(rect.maxX * scale) {
                guard let color = bitmap.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
                brightest = max(brightest, color.greenComponent)
            }
        }
        #expect(brightest > 0.5)
    }

    @Test func selectionCopiesText() throws {
        let view = try makeView(feeding: "hello world\r\nsecond")
        view.selection = TerminalSelection(
            anchor: GridPosition(row: 0, col: 6), head: GridPosition(row: 1, col: 2))
        #expect(view.selection?.text(in: view.session.frame) == "world\nsec")

        view.selection = TerminalSelection(anchor: GridPosition(row: 0, col: 2), granularity: .word)
        #expect(view.selection?.text(in: view.session.frame) == "hello")

        view.selection = TerminalSelection(anchor: GridPosition(row: 1, col: 0), granularity: .line)
        #expect(view.selection?.text(in: view.session.frame) == "second")
    }

    @Test func boxDrawingUsesGlyphGeometry() throws {
        let view = try makeView(feeding: "┌─┐")
        #expect(Set(view.glyphPlan.keys) == [0, 1, 2])
        try view.session.feed(Data("\r\u{1b}[Kabc".utf8))
        #expect(view.glyphPlan.isEmpty)
    }

    @Test func pasteWritesThroughTheSession() throws {
        let view = try makeView()
        var sent = Data()
        view.session.terminal.onInput = { sent.append($0) }
        try view.session.paste("echo hi")
        #expect(String(decoding: sent, as: UTF8.self) == "echo hi")
    }
}
