import CoreGraphics
import Foundation

struct TerminalKittyGraphicsIdentity: Hashable {
    var placementSerial: UInt64
    var imageID: UInt32
    var imageGeneration: UInt64
}

struct TerminalKittyGraphicsImageKey: Hashable {
    var imageID: UInt32
    var imageGeneration: UInt64
}

struct TerminalKittyGraphicsPlacement: Identifiable, Equatable {
    var placementSerial: UInt64
    var imageID: UInt32
    var placementID: UInt32
    var png: Data
    var imageWidth: Int
    var imageHeight: Int
    var imageGeneration: UInt64
    var viewportRow: Int
    var col: Int
    var sourceX: Int
    var sourceY: Int
    var sourceWidth: Int
    var sourceHeight: Int
    var displayCols: Int?
    var displayRows: Int?
    var occupiedCols: Int
    var occupiedRows: Int
    var xOffset: Int
    var yOffset: Int
    var zIndex: Int

    var id: TerminalKittyGraphicsIdentity { identity }

    var identity: TerminalKittyGraphicsIdentity {
        TerminalKittyGraphicsIdentity(
            placementSerial: placementSerial,
            imageID: imageID,
            imageGeneration: imageGeneration
        )
    }

    var imageKey: TerminalKittyGraphicsImageKey {
        TerminalKittyGraphicsImageKey(
            imageID: imageID,
            imageGeneration: imageGeneration
        )
    }

    func bounds(renderConfig: TerminalRenderConfig) -> CGRect {
        let width = displayCols.map { CGFloat($0) * renderConfig.cellWidth }
            ?? CGFloat(sourceWidth)
        let height = displayRows.map { CGFloat($0) * renderConfig.cellHeight }
            ?? CGFloat(sourceHeight)
        return CGRect(
            x: renderConfig.paddingX + CGFloat(col) * renderConfig.cellWidth + CGFloat(xOffset),
            y: renderConfig.paddingY + CGFloat(viewportRow) * renderConfig.cellHeight + CGFloat(yOffset),
            width: width,
            height: height
        )
    }

    func intersects(
        selection: TerminalSelection?,
        cols: Int,
        rows: Int
    ) -> Bool {
        let placementRows = viewportRow..<(viewportRow + max(1, occupiedRows))
        let placementStartCol = col
        let placementEndCol = col + max(1, occupiedCols) - 1
        return selection?
            .rowRanges(cols: cols, rows: rows)
            .contains { range in
                placementRows.contains(range.row)
                    && range.startCol <= placementEndCol
                    && range.endCol >= placementStartCol
            } == true
    }

    static func == (
        lhs: TerminalKittyGraphicsPlacement,
        rhs: TerminalKittyGraphicsPlacement
    ) -> Bool {
        lhs.placementSerial == rhs.placementSerial
            && lhs.imageID == rhs.imageID
            && lhs.placementID == rhs.placementID
            && lhs.imageWidth == rhs.imageWidth
            && lhs.imageHeight == rhs.imageHeight
            && lhs.imageGeneration == rhs.imageGeneration
            && lhs.viewportRow == rhs.viewportRow
            && lhs.col == rhs.col
            && lhs.sourceX == rhs.sourceX
            && lhs.sourceY == rhs.sourceY
            && lhs.sourceWidth == rhs.sourceWidth
            && lhs.sourceHeight == rhs.sourceHeight
            && lhs.displayCols == rhs.displayCols
            && lhs.displayRows == rhs.displayRows
            && lhs.occupiedCols == rhs.occupiedCols
            && lhs.occupiedRows == rhs.occupiedRows
            && lhs.xOffset == rhs.xOffset
            && lhs.yOffset == rhs.yOffset
            && lhs.zIndex == rhs.zIndex
    }

    static func paintOrder(
        _ lhs: TerminalKittyGraphicsPlacement,
        _ rhs: TerminalKittyGraphicsPlacement
    ) -> Bool {
        if lhs.zIndex != rhs.zIndex {
            return lhs.zIndex < rhs.zIndex
        }
        if lhs.imageID != rhs.imageID {
            return lhs.imageID < rhs.imageID
        }
        if lhs.placementID != rhs.placementID {
            return lhs.placementID < rhs.placementID
        }
        return lhs.placementSerial < rhs.placementSerial
    }
}

struct TerminalKittyGraphicsSnapshot {
    var revision: UInt64
    var placements: [TerminalKittyGraphicsPlacement]
}

struct TerminalKittyGraphicsQuerySignature: Equatable {
    var revision: UInt64
    var cols: Int
    var rows: Int
    var displayOffset: Int
    var historySize: Int
}
