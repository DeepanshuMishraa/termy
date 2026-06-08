import Foundation

/// Detects keybind triggers bound more than once. The keybind router resolves a
/// trigger to the *first* matching binding, so any later binding on the same
/// trigger is silently shadowed — surfacing that prevents confusing dead keys.
enum TerminalKeybindConflicts {
    /// Returns the original trigger strings that collide (after normalizing
    /// modifier order and the `secondary`→`cmd` alias).
    static func conflictingTriggers(in keybinds: [TermyKeybindConfiguration]) -> Set<String> {
        var byNormalized: [String: [String]] = [:]
        for keybind in keybinds {
            byNormalized[normalize(keybind.trigger), default: []].append(keybind.trigger)
        }
        var conflicts: Set<String> = []
        for triggers in byNormalized.values where triggers.count > 1 {
            conflicts.formUnion(triggers)
        }
        return conflicts
    }

    /// Canonical form of a trigger so `shift-cmd-d` and `cmd-shift-d`, or
    /// `secondary-k` and `cmd-k`, compare equal.
    static func normalize(_ trigger: String) -> String {
        let parts = trigger.lowercased().split(separator: "-").map(String.init)
        guard let key = parts.last else {
            return trigger.lowercased()
        }
        let modifiers = parts.dropLast().map { $0 == "secondary" ? "cmd" : $0 }
        return (Set(modifiers).sorted() + [key]).joined(separator: "-")
    }
}
