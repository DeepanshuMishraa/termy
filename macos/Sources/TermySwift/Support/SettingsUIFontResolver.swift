import SwiftUI

/// The font the Settings UI should render in. The decision is a plain value so
/// it can be unit-tested without rendering any SwiftUI view.
enum SettingsUIFont: Equatable {
    case system(size: CGFloat)
    case custom(name: String, size: CGFloat)
}

/// Decides which base font the Settings UI uses.
///
/// The Settings UI uses the native macOS system font by default and only
/// switches to the configured `ui_font_family` when the user has explicitly set
/// one. Tabs and the command palette are intentionally NOT routed through this
/// resolver — they always follow `ui_font_family`.
enum SettingsUIFontResolver {
    static func font(family: String, isExplicitlySet: Bool, size: CGFloat) -> SettingsUIFont {
        let trimmed = family.trimmingCharacters(in: .whitespacesAndNewlines)
        guard isExplicitlySet, !trimmed.isEmpty else {
            return .system(size: size)
        }
        return .custom(name: trimmed, size: size)
    }

    /// Maps the decision to a SwiftUI `Font` for the view layer.
    static func swiftUIFont(_ decision: SettingsUIFont) -> Font {
        switch decision {
        case let .system(size):
            return .system(size: size)
        case let .custom(name, size):
            return .custom(name, size: size)
        }
    }
}
