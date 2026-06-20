import SwiftUI

struct TermyUIFontModifier: ViewModifier {
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared
    let size: CGFloat

    func body(content: Content) -> some View {
        let family = configurationStore.configuration.uiFontFamily
        content.font(.custom(family, size: size))
    }
}

extension View {
    func termyUIFont(size: CGFloat = 13) -> some View {
        modifier(TermyUIFontModifier(size: size))
    }
}

/// Like `TermyUIFontModifier` but for the Settings UI: renders in the native
/// macOS system font unless the user explicitly set `ui_font_family`. Kept
/// separate so the main window (tabs, command palette) and onboarding keep
/// following `ui_font_family` unconditionally.
struct TermySettingsUIFontModifier: ViewModifier {
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared
    let size: CGFloat

    func body(content: Content) -> some View {
        let configuration = configurationStore.configuration
        let decision = SettingsUIFontResolver.font(
            family: configuration.uiFontFamily,
            isExplicitlySet: configuration.isUIFontExplicitlySet,
            size: size
        )
        content.font(SettingsUIFontResolver.swiftUIFont(decision))
    }
}

extension View {
    func termySettingsUIFont(size: CGFloat = 13) -> some View {
        modifier(TermySettingsUIFontModifier(size: size))
    }
}
