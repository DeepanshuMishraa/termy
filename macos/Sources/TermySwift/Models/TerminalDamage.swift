import Foundation

/// A contiguous run of changed columns on a single grid row, as reported by the
/// terminal core's damage tracker. Drives partial render-plan rebuilds so a
/// single-cell change (e.g. a cursor blink) does not force the whole grid to be
/// re-laid-out.
struct TerminalDirtySpan: Equatable {
    var row: Int
    var leftCol: Int
    var rightCol: Int
}

/// The shape of the change reported by `termy_terminal_take_damage`.
///
/// - `none`: nothing changed since the last poll; the cached render plan is reused.
/// - `full`: the entire visible grid changed (resize, scroll, clear); rebuild all rows.
/// - `partial`: only the listed rows changed; rebuild just those rows.
enum TerminalDamage: Equatable {
    case none
    case full
    case partial([TerminalDirtySpan])

    /// Whether anything changed and a redraw is warranted.
    var hasChanges: Bool {
        switch self {
        case .none:
            return false
        case .full:
            return true
        case let .partial(spans):
            return !spans.isEmpty
        }
    }
}
