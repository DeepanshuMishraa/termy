import AppKit
import Combine

/// A selectable Dock / Cmd-Tab app icon. The `resourceName` is the PNG basename
/// bundled into the app's `Contents/Resources` by `script/build_and_run.sh`.
struct AppLogo: Identifiable, Hashable {
    let id: String
    let label: String
    let resourceName: String

    static let all: [AppLogo] = [
        AppLogo(id: "termy", label: "Termy Icon", resourceName: "TermyIcon"),
        AppLogo(id: "classic", label: "Classic", resourceName: "termy_old_icon"),
    ]

    static let `default` = all[1]
}

/// Owns the currently selected app logo from shared config and pushes it to the
/// live Dock icon (`NSApp.applicationIconImage`).
@MainActor
final class AppLogoManager: ObservableObject {
    static let shared = AppLogoManager()

    @Published private(set) var selectedID: String
    private var imageCache: [String: NSImage] = [:]

    private init() {
        selectedID = Self.logoID(for: TermyConfigurationStore.shared.configuration.native.appIcon)
    }

    var selected: AppLogo {
        AppLogo.all.first { $0.id == selectedID } ?? .default
    }

    /// Loads a logo image from the app bundle's Resources.
    func image(for logo: AppLogo) -> NSImage? {
        if let cached = imageCache[logo.id] {
            return cached
        }
        let image: NSImage?
        if let pngURL = Bundle.main.url(forResource: logo.resourceName, withExtension: "png") {
            image = NSImage(contentsOf: pngURL)
        } else if let icnsURL = Bundle.main.url(forResource: logo.resourceName, withExtension: "icns") {
            image = NSImage(contentsOf: icnsURL)
        } else {
            image = nil
        }
        if let image {
            imageCache[logo.id] = image
        }
        return image
    }

    /// The logo already baked into the bundle as `CFBundleIconFile`; the Dock
    /// shows it without any in-process image.
    private static let bundleIconResourceName = "TermyIcon"

    /// Applies the selected logo to the running app's Dock / Cmd-Tab icon.
    /// Called on launch and whenever the selection changes.
    ///
    /// Setting `NSApp.applicationIconImage` makes AppKit render and retain a
    /// process-lifetime snapshot at `image.size × screen scale` in deep color
    /// (a 1024 pt image costs a 2048×2048×8B ≈ 32 MB bitmap). So the bundle's
    /// own icon is restored with `nil` instead of re-assigned, and custom
    /// logos are capped at 512 pt — the source PNGs are 1024 px, so the
    /// snapshot stays pixel-identical at 4× less memory.
    func applyToDock() {
        guard selected.resourceName != Self.bundleIconResourceName else {
            NSApp.applicationIconImage = nil
            return
        }
        guard let image = image(for: selected), let dockImage = image.copy() as? NSImage else {
            return
        }
        let maxSide: CGFloat = 512
        if dockImage.size.width > maxSide || dockImage.size.height > maxSide {
            let scale = maxSide / max(dockImage.size.width, dockImage.size.height)
            dockImage.size = NSSize(
                width: dockImage.size.width * scale,
                height: dockImage.size.height * scale
            )
        }
        NSApp.applicationIconImage = dockImage
    }

    func reloadFromConfig() {
        let nextID = Self.logoID(for: TermyConfigurationStore.shared.reload().native.appIcon)
        guard nextID != selectedID else {
            return
        }
        selectedID = nextID
        applyToDock()
    }

    private static func logoID(for appIcon: TermyAppIcon) -> String {
        switch appIcon {
        case .default:
            return "termy"
        case .old:
            return "classic"
        }
    }
}
