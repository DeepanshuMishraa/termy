import AppKit

/// An `NSWindow` subclass that relocates AppKit's native window tab bar onto the
/// same row as the traffic-light buttons — the "titlebar tabs" look used by
/// Ghostty (`macos-titlebar-style = "tabs"`), Safari, and friends.
///
/// AppKit normally adds the window tab bar as an `NSTitlebarAccessoryViewController`
/// laid out as a full-width strip *below* the titlebar. Hiding the window title
/// only collapses the (empty) title row; the tab strip still sits on its own
/// line. To merge it into the traffic-light row we (1) install an `NSToolbar`
/// with `toolbarStyle = .unifiedCompact` so the titlebar and toolbar share one
/// band, and (2) flip the tab accessory's `layoutAttribute` to `.right` and
/// constrain its clip view into that unified `NSToolbarView`, leaving a gutter on
/// the left for the window buttons.
///
/// When there is only one tab (no tab bar), a centered toolbar item shows the
/// window title instead — matching Ghostty. The title item collapses to nothing
/// while the tab bar is present.
///
/// This is the macOS 26 (Tahoe) technique, ported from Ghostty's
/// `TitlebarTabsTahoeTerminalWindow`. It relies on private AppKit view-class
/// names (`NSTitlebarView`, `NSToolbarView`, `NSTabBar`,
/// `NSTitlebarAccessoryClipView`) reached through KVC and class-name walks. Every
/// lookup is guarded and early-returns on mismatch, so on an OS where the
/// internal layout changes the merge simply no-ops and the tab bar falls back to
/// the standard strip below the traffic lights — it never crashes.
final class TitlebarTabsWindow: NSWindow, NSToolbarDelegate {
    static let tabBarIdentifier = NSUserInterfaceItemIdentifier("_termyTabBar")

    /// Left gutter reserved for the traffic-light buttons.
    private static let windowButtonsGutter: CGFloat = 70

    /// The centered window-title label, shown only when there is no tab bar.
    private let titleField = TitlebarTitleField(labelWithString: "")

    /// Observes the native tab bar's frame so we can re-pin our constraints when
    /// AppKit resizes it (appearance changes, tab open/close clear our layout).
    /// Only ever touched on the main thread; `nonisolated(unsafe)` lets `deinit`
    /// and the (main-queue) notification block reach it under Swift 6 isolation.
    private nonisolated(unsafe) var tabBarObserver: NSObjectProtocol? {
        didSet {
            guard let oldValue else { return }
            NotificationCenter.default.removeObserver(oldValue)
        }
    }

    deinit {
        if let tabBarObserver {
            NotificationCenter.default.removeObserver(tabBarObserver)
        }
    }

    /// Enables the titlebar-tabs treatment. Installing the toolbar makes the
    /// unified-compact titlebar band exist; `setupTabBar` then pulls the native
    /// tab bar into it once AppKit has created it.
    var titlebarTabs = false {
        didSet {
            titleVisibility = titlebarTabs ? .hidden : .visible
            if titlebarTabs {
                generateToolbar()
                setupTabBar()
            } else {
                toolbar = nil
                tabBarObserver = nil
            }
        }
    }

    // Assigning `title` re-reveals the native title view on macOS 15+/26; re-hide
    // it while titlebar tabs are active (our custom toolbar item shows it instead).
    override var title: String {
        didSet {
            guard titlebarTabs else { return }
            titleVisibility = .hidden
            titleField.stringValue = title
        }
    }

    private func generateToolbar() {
        guard toolbar == nil else { return }
        let toolbar = NSToolbar(identifier: "TermyTitlebarTabs")
        toolbar.delegate = self
        toolbar.showsBaselineSeparator = false
        toolbar.centeredItemIdentifiers.insert(.termyTitle)
        self.toolbar = toolbar
        toolbarStyle = .unifiedCompact
        titleField.stringValue = title
    }

    // MARK: NSWindow

    override func becomeMain() {
        super.becomeMain()
        titleField.textColor = .labelColor
        // AppKit only attaches the live NSTabBar to whichever window in the group
        // is main, moving it on focus changes — so re-pin it every time we gain
        // main. `setupTabBar` is idempotent.
        if titlebarTabs {
            // Defer a tick so the sync sees the post-transition tab-bar state
            // (e.g. after closing the key tab, this window only learns it is now
            // alone via becoming main — there is no accessory-removal callback).
            DispatchQueue.main.async { [weak self] in
                self?.syncTitleVisibility()
            }
            setupTabBar()
        }
    }

    /// True when the native tab bar is occupying the titlebar row. We gate the
    /// centered title on the bar's *visibility*, not the tab count: a force-shown
    /// bar (System Settings "Prefer tabs: Always", or View ▸ Show Tab Bar) leaves
    /// a lone window at count 1 with a visible bar, so a count-based gate would
    /// draw the title on top of the strip. `isTabBarVisible` also stays false for
    /// a leftover empty bar, preserving the no-blank-title intent.
    private var isTabBarOccupyingRow: Bool {
        tabGroup?.isTabBarVisible ?? false
    }

    /// Show the centered window title only when the tab bar is not on the row.
    private func syncTitleVisibility() {
        titleField.isHidden = isTabBarOccupyingRow
    }

    override func resignMain() {
        super.resignMain()
        titleField.textColor = .tertiaryLabelColor
    }

    override func sendEvent(_ event: NSEvent) {
        if event.type == .leftMouseDown,
           event.clickCount == 2,
           titlebarTabs,
           shouldHandleTitlebarDoubleClick(at: event.locationInWindow)
        {
            performTitlebarDoubleClickAction()
            return
        }
        super.sendEvent(event)
    }

    /// Matches AppKit's window tab bar accessory. AppKit first attaches an empty
    /// `.bottom` `NSView` and only later fills it with an `NSTabBar`, so both
    /// shapes are treated as the tab bar (mirrors Ghostty's detection).
    private func isTabBar(_ controller: NSTitlebarAccessoryViewController) -> Bool {
        if controller.identifier == Self.tabBarIdentifier {
            return true
        }
        guard controller.identifier == nil else {
            return false
        }
        if controller.view.contains(className: "NSTabBar") {
            return true
        }
        return controller.layoutAttribute == .bottom
            && controller.view.className == "NSView"
            && controller.view.subviews.isEmpty
    }

    override func addTitlebarAccessoryViewController(_ controller: NSTitlebarAccessoryViewController) {
        guard titlebarTabs, isTabBar(controller) else {
            super.addTitlebarAccessoryViewController(controller)
            return
        }

        // Must happen BEFORE super, or AppKit raises a layout assertion: a
        // `.right` accessory is laid out inside the toolbar row instead of as a
        // separate strip below it.
        controller.layoutAttribute = .right
        controller.identifier = Self.tabBarIdentifier
        titleVisibility = .hidden
        super.addTitlebarAccessoryViewController(controller)

        // Wait a tick: doing this synchronously races AppKit's own accessory
        // layout and produces a truncated tab bar on restored windows.
        DispatchQueue.main.async { [weak self] in
            self?.setupTabBar()
        }
    }

    override func removeTitlebarAccessoryViewController(at index: Int) {
        let wasTabBar = titlebarAccessoryViewControllers[safe: index]?.identifier == Self.tabBarIdentifier
        super.removeTitlebarAccessoryViewController(at: index)
        if wasTabBar {
            tabBarObserver = nil
            // Back to a single tab: show the centered title again. Defer a tick so
            // `tabbedWindows` reflects the post-removal count, not the stale group.
            DispatchQueue.main.async { [weak self] in
                self?.syncTitleVisibility()
            }
        }
    }

    // MARK: Tab Bar Setup

    /// Pull the native `NSTabBar` up into the unified toolbar row. Idempotent: the
    /// `tabBarObserver` guard means repeated calls (from `becomeMain`, focus
    /// changes, the accessory hook) are cheap no-ops once wired up.
    func setupTabBar() {
        guard tabBarObserver == nil else { return }
        guard let titlebarView,
              let tabBarView = self.tabBarView
        else {
            // No tab bar present (single tab): show the centered title.
            syncTitleVisibility()
            return
        }

        // The clip view is the accessory's parent; its class name shifts across
        // OS versions, so try both known names.
        guard let clipView = tabBarView.firstSuperview(withClassName: "NSTitlebarAccessoryClipView")
            ?? tabBarView.firstSuperview(withClassName: "NSTitlebarAccessoryContainerView")
        else {
            return
        }
        guard let accessoryView = clipView.subviews.first,
              let toolbarView = titlebarView.firstDescendant(withClassName: "NSToolbarView")
        else {
            return
        }

        // A tab bar exists: hide the centered title so it doesn't overlap (unless
        // this is a lone window with a leftover empty bar).
        syncTitleVisibility()

        // Keep the tab bar from being vertically stretched: AppKit sizes the new
        // tab button square, so match the bar height to its width.
        if let newTabButton = titlebarView.firstDescendant(withClassName: "NSTabBarNewTabButton") {
            tabBarView.frame.size.height = newTabButton.frame.width
        }

        clipView.translatesAutoresizingMaskIntoConstraints = false
        accessoryView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            clipView.leftAnchor.constraint(equalTo: toolbarView.leftAnchor, constant: Self.windowButtonsGutter),
            clipView.rightAnchor.constraint(equalTo: toolbarView.rightAnchor),
            clipView.topAnchor.constraint(equalTo: toolbarView.topAnchor, constant: 2),
            clipView.heightAnchor.constraint(equalTo: toolbarView.heightAnchor),
            accessoryView.leftAnchor.constraint(equalTo: clipView.leftAnchor),
            accessoryView.rightAnchor.constraint(equalTo: clipView.rightAnchor),
            accessoryView.topAnchor.constraint(equalTo: clipView.topAnchor),
            accessoryView.heightAnchor.constraint(equalTo: clipView.heightAnchor),
        ])
        clipView.needsLayout = true
        accessoryView.needsLayout = true

        // The tab bar can resize (appearance change, tab add/remove) and clear our
        // constraints. Observe its frame, drop the observer, and re-run setup once
        // it settles to avoid constraint conflicts.
        tabBarView.postsFrameChangedNotifications = true
        tabBarObserver = NotificationCenter.default.addObserver(
            forName: NSView.frameDidChangeNotification,
            object: tabBarView,
            queue: .main
        ) { [weak self] _ in
            // `queue: .main` guarantees we're on the main thread here.
            MainActor.assumeIsolated {
                guard let self else { return }
                self.tabBarObserver = nil
                self.setupTabBar()
            }
        }
    }

    // MARK: NSToolbarDelegate

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.termyTitle, .flexibleSpace, .space]
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        // Flexible spaces on both sides keep the title centered as it grows.
        [.flexibleSpace, .termyTitle, .flexibleSpace]
    }

    func toolbar(
        _ toolbar: NSToolbar,
        itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier,
        willBeInsertedIntoToolbar flag: Bool
    ) -> NSToolbarItem? {
        guard itemIdentifier == .termyTitle else {
            return NSToolbarItem(itemIdentifier: itemIdentifier)
        }
        let item = NSToolbarItem(itemIdentifier: .termyTitle)
        item.view = titleField
        item.visibilityPriority = .user
        item.isEnabled = false
        // Avoids the glass capsule background NSToolbar draws around custom views.
        item.isBordered = false
        // Size to the text and let the flexible spaces center it. Hugging low so
        // the field can grow toward (but not past) the available width.
        titleField.translatesAutoresizingMaskIntoConstraints = false
        titleField.setContentHuggingPriority(.defaultLow, for: .horizontal)
        titleField.setContentCompressionResistancePriority(.defaultHigh, for: .horizontal)
        titleField.heightAnchor.constraint(equalToConstant: 22).isActive = true
        return item
    }

    private func shouldHandleTitlebarDoubleClick(at windowPoint: NSPoint) -> Bool {
        guard isPointInTitlebarBand(windowPoint) else {
            return false
        }
        if let hitView = contentView?.rootView.hitTest(windowPoint),
           hitView.isInteractiveTitlebarTabControl
        {
            return false
        }
        return true
    }

    private func isPointInTitlebarBand(_ windowPoint: NSPoint) -> Bool {
        let contentMaxY = contentLayoutRect.maxY
        if windowPoint.y >= contentMaxY {
            return true
        }

        if let titlebarView {
            let titlebarFrame = titlebarView.convert(titlebarView.bounds, to: nil)
            if titlebarFrame.contains(windowPoint) {
                return true
            }
        }

        if let toolbarView = titlebarView?.firstDescendant(withClassName: "NSToolbarView") {
            let toolbarFrame = toolbarView.convert(toolbarView.bounds, to: nil)
            if toolbarFrame.contains(windowPoint) {
                return true
            }
        }

        return false
    }

    private func performTitlebarDoubleClickAction() {
        switch UserDefaults.standard.string(forKey: "AppleActionOnDoubleClick")?.lowercased() {
        case "minimize":
            performMiniaturize(nil)
        case "none":
            return
        default:
            zoom(nil)
        }
    }
}

private extension NSView {
    var isInteractiveTitlebarTabControl: Bool {
        var view: NSView? = self
        while let current = view {
            if current.className == "NSTabBarButton"
                || current.className == "NSTabBarNewTabButton"
                || current.className == "NSButton"
            {
                return true
            }
            if current.className == "NSTitlebarView"
                || current.className == "NSToolbarView"
            {
                return false
            }
            view = current.superview
        }
        return false
    }
}

/// A non-interactive, vertically-centered titlebar label that passes clicks
/// through so titlebar dragging and double-click-to-zoom keep working.
private final class TitlebarTitleField: NSTextField {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        isEditable = false
        isSelectable = false
        isBordered = false
        drawsBackground = false
        alignment = .center
        lineBreakMode = .byTruncatingTail
        cell?.truncatesLastVisibleLine = true
        font = .titleBarFont(ofSize: NSFont.systemFontSize)
        textColor = .labelColor
    }
}

private extension NSToolbarItem.Identifier {
    static let termyTitle = NSToolbarItem.Identifier("TermyTitle")
}

// MARK: - Private AppKit view-hierarchy access

private extension NSWindow {
    /// The private `NSTitlebarView`, reached via KVC on the theme frame. Guarded
    /// by `responds(to:)` so it returns nil instead of crashing if the selector
    /// ever disappears.
    var titlebarView: NSView? {
        guard let themeFrame = contentView?.rootView,
              themeFrame.responds(to: Selector(("titlebarView")))
        else {
            return nil
        }
        return themeFrame.value(forKey: "titlebarView") as? NSView
    }

    /// The private `NSTabBar` view, if the window currently hosts one.
    var tabBarView: NSView? {
        titlebarView?.firstDescendant(withClassName: "NSTabBar")
    }
}

private extension NSView {
    /// The topmost ancestor (the `NSThemeFrame`).
    var rootView: NSView {
        var root = self
        while let superview = root.superview {
            root = superview
        }
        return root
    }

    /// True if `self` or any descendant is of the given AppKit view class.
    /// Uses `NSObject.className`, the Objective-C runtime class name.
    func contains(className name: String) -> Bool {
        if className == name {
            return true
        }
        return subviews.contains { $0.contains(className: name) }
    }

    /// The nearest ancestor of the given AppKit view class.
    func firstSuperview(withClassName name: String) -> NSView? {
        guard let superview else { return nil }
        if superview.className == name {
            return superview
        }
        return superview.firstSuperview(withClassName: name)
    }

    /// The first descendant (depth-first) of the given AppKit view class.
    func firstDescendant(withClassName name: String) -> NSView? {
        for subview in subviews {
            if subview.className == name {
                return subview
            }
            if let found = subview.firstDescendant(withClassName: name) {
                return found
            }
        }
        return nil
    }
}
