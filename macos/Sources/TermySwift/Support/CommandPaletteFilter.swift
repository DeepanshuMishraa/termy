import Foundation

/// Result of matching a palette query against one command.
struct CommandPaletteMatch: Equatable {
    /// Higher ranks earlier in the palette list.
    var score: Int
    /// Character offsets in the title that matched, for highlighting.
    var matchedTitleIndices: [Int]
}

/// Fuzzy matcher for command palette entries: a query matches when its
/// characters appear in order in the title (or as a substring of the action
/// id). Consecutive runs and word-start hits rank higher, so "spr" finds
/// "Split Right" above scattered matches and "nt" puts "New Tab" first.
enum CommandPaletteFilter {
    static func match(query: String, title: String, action: String) -> CommandPaletteMatch? {
        let needle = Array(query.lowercased())
        guard !needle.isEmpty else {
            return CommandPaletteMatch(score: 0, matchedTitleIndices: [])
        }

        if let match = subsequenceMatch(needle: needle, haystack: Array(title.lowercased())) {
            return match
        }

        // Fall back to the action id ("clear_buffer", "open_search") so power
        // users can search by internal command names. Ranked below any title
        // match and with no title highlight.
        if action.lowercased().contains(query.lowercased()) {
            return CommandPaletteMatch(score: 1, matchedTitleIndices: [])
        }
        return nil
    }

    /// Greedy left-to-right subsequence match with ranking bonuses:
    /// +10 per matched character, +15 when the hit starts a word, +20 when it
    /// directly continues the previous hit, minus a small penalty for late
    /// first hits so prefix matches rank above mid-word ones.
    private static func subsequenceMatch(
        needle: [Character],
        haystack: [Character]
    ) -> CommandPaletteMatch? {
        var indices: [Int] = []
        var score = 0
        var position = 0
        var lastMatch = -2

        for character in needle {
            var found = false
            while position < haystack.count {
                if haystack[position] == character {
                    score += 10
                    if position == 0 || haystack[position - 1] == " " {
                        score += 15
                    }
                    if position == lastMatch + 1 {
                        score += 20
                    }
                    indices.append(position)
                    lastMatch = position
                    position += 1
                    found = true
                    break
                }
                position += 1
            }
            if !found {
                return nil
            }
        }

        score -= min(indices.first ?? 0, 10)
        return CommandPaletteMatch(score: score, matchedTitleIndices: indices)
    }
}
