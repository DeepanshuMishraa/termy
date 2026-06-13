import Foundation

/// A parsed tmux window layout (from `#{window_layout}` / `%layout-change`),
/// used to map a tmux pane arrangement onto a `TerminalPane` tree.
indirect enum TmuxLayoutNode: Equatable {
    case pane(id: Int, width: Int, height: Int, x: Int, y: Int)
    /// Panes arranged left-to-right (tmux `{ }`).
    case horizontal([TmuxLayoutNode])
    /// Panes stacked top-to-bottom (tmux `[ ]`).
    case vertical([TmuxLayoutNode])
}

enum TmuxLayout {
    /// Parses a tmux layout string, e.g. `a1b2,80x24,0,0{40x24,0,0,1,39x24,41,0,2}`.
    /// The leading checksum is ignored.
    static func parse(_ layout: String) -> TmuxLayoutNode? {
        guard let comma = layout.firstIndex(of: ",") else {
            return nil
        }
        let geometry = Array(layout[layout.index(after: comma)...])
        var cursor = 0
        let node = parseNode(geometry, &cursor)
        // A valid layout consumes the whole geometry; trailing garbage is rejected.
        return cursor == geometry.count ? node : nil
    }

    private static func parseNode(_ chars: [Character], _ cursor: inout Int) -> TmuxLayoutNode? {
        guard let width = parseInt(chars, &cursor), consume(chars, &cursor, "x"),
              let height = parseInt(chars, &cursor), consume(chars, &cursor, ","),
              let x = parseInt(chars, &cursor), consume(chars, &cursor, ","),
              let y = parseInt(chars, &cursor)
        else {
            return nil
        }

        guard cursor < chars.count else {
            return nil
        }
        switch chars[cursor] {
        case "{":
            return parseChildren(chars, &cursor, open: "{", close: "}").map(TmuxLayoutNode.horizontal)
        case "[":
            return parseChildren(chars, &cursor, open: "[", close: "]").map(TmuxLayoutNode.vertical)
        case ",":
            cursor += 1
            guard let id = parseInt(chars, &cursor) else {
                return nil
            }
            return .pane(id: id, width: width, height: height, x: x, y: y)
        default:
            return nil
        }
    }

    private static func parseChildren(
        _ chars: [Character],
        _ cursor: inout Int,
        open: Character,
        close: Character
    ) -> [TmuxLayoutNode]? {
        guard consume(chars, &cursor, open) else {
            return nil
        }
        var children: [TmuxLayoutNode] = []
        while true {
            guard let child = parseNode(chars, &cursor) else {
                return nil
            }
            children.append(child)
            guard cursor < chars.count else {
                return nil
            }
            if chars[cursor] == "," {
                cursor += 1
                continue
            }
            if chars[cursor] == close {
                cursor += 1
                return children
            }
            return nil
        }
    }

    private static func parseInt(_ chars: [Character], _ cursor: inout Int) -> Int? {
        let start = cursor
        while cursor < chars.count, chars[cursor].isNumber {
            cursor += 1
        }
        guard cursor > start else {
            return nil
        }
        return Int(String(chars[start..<cursor]))
    }

    private static func consume(_ chars: [Character], _ cursor: inout Int, _ expected: Character) -> Bool {
        guard cursor < chars.count, chars[cursor] == expected else {
            return false
        }
        cursor += 1
        return true
    }
}
