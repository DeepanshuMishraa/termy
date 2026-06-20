import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct NativeTabChromeView: View {
    weak var window: NSWindow?
    let configuration: TermyAppConfiguration
    let renderConfig: TerminalRenderConfig

    @State private var hoveredTabID: ObjectIdentifier?
    @State private var modifierFlags: NSEvent.ModifierFlags = []
    @State private var modifierMonitor: Any?
    @State private var refreshToken = 0
    @State private var renameRequest: NativeTabRenameRequest?
    @State private var renameText = ""
    @State private var renderedStructure: [TabStructureSignature] = []
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var tabs: [NativeTabDescriptor] {
        _ = refreshToken
        return NativeTabWindowManager.shared.tabDescriptors(for: window)
    }

    /// Shared spring for tab entrances, removals, and width changes —
    /// approximates the system tab bar's feel.
    static let chromeAnimation: Animation = .spring(response: 0.3, dampingFraction: 0.85)

    /// The chrome spring, or `nil` when the system requests reduced motion.
    private var motion: Animation? {
        reduceMotion ? nil : Self.chromeAnimation
    }

    /// Quick hover crossfade, suppressed under reduced motion.
    private var hoverMotion: Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.12)
    }

    private var selectedTabID: NativeTabDescriptor.ID? {
        tabs.first(where: { $0.isSelected })?.id
    }

    /// Geometry-affecting snapshot of the tab list, used to decide whether a
    /// refresh should animate.
    private var currentTabStructure: [TabStructureSignature] {
        tabs.map {
            TabStructureSignature(id: $0.id, isSelected: $0.isSelected, isPinned: $0.isPinned)
        }
    }

    var body: some View {
        if shouldRender {
            switch configuration.native.tabBarPosition {
            case .top:
                NativeTabEntrance(
                    animatesEntrance: NativeTabWindowManager.shared.shouldAnimateBarEntrance(for: window),
                    collapses: .vertical,
                    alignment: .bottom
                ) {
                    horizontalChrome
                }
                .transition(.move(edge: .top).combined(with: .opacity))
            case .right:
                NativeTabEntrance(
                    animatesEntrance: NativeTabWindowManager.shared.shouldAnimateBarEntrance(for: window),
                    collapses: .horizontal,
                    alignment: .trailing
                ) {
                    verticalChrome
                }
                .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
    }

    private var shouldRender: Bool {
        !tabs.isEmpty && (!configuration.native.autoHideTabbar || tabs.count > 1)
    }

    private var horizontalChrome: some View {
        HStack(spacing: 6) {
            if configuration.native.showTermyInTitlebar {
                Text("Termy")
                    .font(uiFont(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.leading, 10)
            }

            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 4) {
                        ForEach(tabs) { tab in
                            NativeTabEntrance(
                                animatesEntrance: NativeTabWindowManager.shared.shouldAnimateTabEntrance(for: tab.id),
                                collapses: .horizontal,
                                alignment: .leading
                            ) {
                                tabButton(tab, axis: .horizontal)
                            }
                            .transition(tabRemovalTransition(collapses: .horizontal))
                        }
                    }
                    .padding(.horizontal, 6)
                }
                .onChange(of: selectedTabID) { _, _ in
                    scrollSelectionIntoView(proxy, animated: true)
                }
                .onAppear {
                    renderedStructure = currentTabStructure
                    scrollSelectionIntoView(proxy, animated: false)
                }
            }

            Button {
                NativeTabWindowManager.shared.openNativeTab()
            } label: {
                Image(systemName: "plus")
                    .frame(width: 22, height: 22)
            }
            .buttonStyle(.plain)
            .help("New Tab")
            .padding(.trailing, 8)
        }
        .frame(height: 34)
        .background(chromeBackground)
        .overlay(alignment: .bottom) {
            Divider()
                .overlay(chromeDivider)
        }
        .modifierFlagsTracking($modifierFlags, monitor: $modifierMonitor)
        .onReceive(NotificationCenter.default.publisher(for: .termyNativeTabsChanged)) { _ in
            handleTabsChanged()
        }
        .sheet(item: $renameRequest) { request in
            RenameNativeTabSheet(
                title: $renameText,
                initialTitle: request.title,
                onCancel: {
                    renameRequest = nil
                },
                onSave: {
                    NativeTabWindowManager.shared.renameNativeTab(request.descriptor, title: renameText)
                    renameRequest = nil
                }
            )
        }
    }

    private var verticalChrome: some View {
        VStack(spacing: 6) {
            if configuration.native.showTermyInTitlebar {
                Text("Termy")
                    .font(uiFont(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.top, 10)
            }

            Button {
                NativeTabWindowManager.shared.openNativeTab()
            } label: {
                Image(systemName: "plus")
                    .frame(width: 24, height: 24)
            }
            .buttonStyle(.plain)
            .help("New Tab")

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    LazyVStack(spacing: 5) {
                        ForEach(tabs) { tab in
                            NativeTabEntrance(
                                animatesEntrance: NativeTabWindowManager.shared.shouldAnimateTabEntrance(for: tab.id),
                                collapses: .vertical,
                                alignment: .top
                            ) {
                                tabButton(tab, axis: .vertical)
                            }
                            .transition(tabRemovalTransition(collapses: .vertical))
                        }
                    }
                    .padding(6)
                }
                .onChange(of: selectedTabID) { _, _ in
                    scrollSelectionIntoView(proxy, animated: true)
                }
                .onAppear {
                    renderedStructure = currentTabStructure
                    scrollSelectionIntoView(proxy, animated: false)
                }
            }
        }
        .frame(width: 164)
        .background(chromeBackground)
        .overlay(alignment: .leading) {
            Divider()
                .overlay(chromeDivider)
        }
        .modifierFlagsTracking($modifierFlags, monitor: $modifierMonitor)
        .onReceive(NotificationCenter.default.publisher(for: .termyNativeTabsChanged)) { _ in
            handleTabsChanged()
        }
        .sheet(item: $renameRequest) { request in
            RenameNativeTabSheet(
                title: $renameText,
                initialTitle: request.title,
                onCancel: {
                    renameRequest = nil
                },
                onSave: {
                    NativeTabWindowManager.shared.renameNativeTab(request.descriptor, title: renameText)
                    renameRequest = nil
                }
            )
        }
    }

    private func tabButton(_ tab: NativeTabDescriptor, axis: Axis) -> some View {
        HStack(spacing: 6) {
            if tab.isPinned {
                Image(systemName: "pin.fill")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(tab.isSelected ? Color.accentColor : Color.secondary)
                    .help("Pinned")
            }

            if shouldShowSwitchHint(for: tab) {
                Text("\(tab.index + 1)")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .frame(width: 14)
            }

            Text(tab.title)
                .font(uiFont(size: 12, weight: tab.isSelected ? .semibold : .regular))
                .lineLimit(1)

            Spacer(minLength: 4)

            if shouldShowCloseButton(for: tab) {
                Button {
                    NativeTabWindowManager.shared.closeNativeTab(tab)
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 10, weight: .semibold))
                        .frame(width: 16, height: 16)
                }
                .buttonStyle(.plain)
                .help("Close Tab")
            }
        }
        .padding(.horizontal, 8)
        .frame(width: tabWidth(for: tab, axis: axis), height: 24)
        .background(tabBackground(for: tab), in: RoundedRectangle(cornerRadius: 6))
        .contentShape(Rectangle())
        .onHover { hovering in
            withAnimation(hoverMotion) {
                hoveredTabID = hovering ? tab.id : nil
            }
        }
        .onTapGesture {
            NativeTabWindowManager.shared.selectNativeTab(tab)
        }
        .onDrag {
            NSItemProvider(object: String(tab.index) as NSString)
        }
        .onDrop(of: [.plainText], isTargeted: nil) { providers in
            handleTabDrop(providers, onto: tab)
        }
        .contextMenu {
            Button(tab.isPinned ? "Unpin Tab" : "Pin Tab") {
                NativeTabWindowManager.shared.setNativeTabPinned(tab, pinned: !tab.isPinned)
            }
            Button("Rename Tab...") {
                beginRename(tab)
            }
            if tab.hasManualTitle {
                Button("Clear Custom Name") {
                    NativeTabWindowManager.shared.renameNativeTab(tab, title: "")
                }
            }
            Divider()
            Button("Close Tab") {
                NativeTabWindowManager.shared.closeNativeTab(tab)
            }
            .disabled(tab.isPinned)
        }
    }

    private func shouldShowCloseButton(for tab: NativeTabDescriptor) -> Bool {
        if tab.isPinned {
            return false
        }
        switch configuration.native.tabCloseVisibility {
        case .always:
            return true
        case .hover:
            return hoveredTabID == tab.id
        case .activeHover:
            return tab.isSelected || hoveredTabID == tab.id
        }
    }

    private func shouldShowSwitchHint(for tab: NativeTabDescriptor) -> Bool {
        configuration.native.tabSwitchModifierHints
            && modifierFlags.contains(.command)
            && tab.index < 9
    }

    private func tabWidth(for tab: NativeTabDescriptor, axis: Axis) -> CGFloat? {
        if axis == .vertical {
            return nil
        }

        switch configuration.native.tabWidthMode {
        case .stable:
            return 148
        case .activeGrow:
            return tab.isSelected ? 210 : 126
        case .activeGrowSticky:
            return tab.isSelected ? 210 : 148
        case .uniform:
            return 156
        }
    }

    private func uiFont(size: CGFloat, weight: Font.Weight) -> Font {
        .custom(configuration.uiFontFamily, size: size).weight(weight)
    }

    private var chromeBackground: Color {
        Color(nsColor: TerminalWindowChromeState(
            title: "",
            isFocused: true,
            background: renderConfig.background,
            backgroundOpacity: renderConfig.backgroundOpacity,
            backgroundBlur: renderConfig.backgroundBlur
        ).chromeBackgroundColor)
    }

    private var chromeDivider: Color {
        renderConfig.foreground.swiftUIColor.opacity(0.12)
    }

    private func tabBackground(for tab: NativeTabDescriptor) -> Color {
        if tab.isSelected {
            return renderConfig.foreground.swiftUIColor.opacity(0.12)
        }
        if hoveredTabID == tab.id {
            return renderConfig.foreground.swiftUIColor.opacity(0.07)
        }
        return Color.clear
    }

    private func beginRename(_ tab: NativeTabDescriptor) {
        renameText = tab.title
        renameRequest = NativeTabRenameRequest(descriptor: tab)
    }

    /// Resolves the dragged tab (carried as its index) and reorders it to land
    /// where `target` currently sits.
    private func handleTabDrop(_ providers: [NSItemProvider], onto target: NativeTabDescriptor) -> Bool {
        guard let provider = providers.first else {
            return false
        }
        _ = provider.loadObject(ofClass: NSString.self) { value, _ in
            guard let sourceIndex = (value as? String).flatMap(Int.init) else {
                return
            }
            Task { @MainActor in
                let current = NativeTabWindowManager.shared.tabDescriptors(for: window)
                guard let source = current.first(where: { $0.index == sourceIndex }) else {
                    return
                }
                NativeTabWindowManager.shared.moveNativeTab(source, toIndex: target.index)
            }
        }
        return true
    }

    /// Refreshes the tab list, animating only when the structure (count, order,
    /// selection, pin state) changed. Programs can rewrite their title many
    /// times a second; routing those through the spring would churn the bar, so
    /// title-only updates refresh without animation.
    private func handleTabsChanged() {
        let structure = currentTabStructure
        let isStructural = structure != renderedStructure
        renderedStructure = structure
        withAnimation(isStructural ? motion : nil) {
            refreshToken &+= 1
        }
    }

    /// Scrolls the selected tab into view — e.g. after `cmd-9` selects a tab
    /// currently clipped by the overflow scroll view.
    private func scrollSelectionIntoView(_ proxy: ScrollViewProxy, animated: Bool) {
        guard let selectedTabID else {
            return
        }
        if animated {
            withAnimation(motion) {
                proxy.scrollTo(selectedTabID)
            }
        } else {
            proxy.scrollTo(selectedTabID)
        }
    }

    /// Closing a tab collapses it along the bar's axis so neighbors slide
    /// into the freed space, mirroring the entrance animation.
    private func tabRemovalTransition(collapses: Axis) -> AnyTransition {
        .asymmetric(
            insertion: .identity,
            removal: .modifier(
                active: NativeTabCollapse(collapses: collapses, isCollapsed: true),
                identity: NativeTabCollapse(collapses: collapses, isCollapsed: false)
            )
        )
    }
}

/// Collapses a tab (or the whole bar) to zero size along one axis. Used for
/// removal transitions and as the starting state of entrance animations.
private struct NativeTabCollapse: ViewModifier {
    let collapses: Axis
    let isCollapsed: Bool

    func body(content: Content) -> some View {
        content
            .frame(
                width: collapses == .horizontal && isCollapsed ? 0 : nil,
                height: collapses == .vertical && isCollapsed ? 0 : nil,
                alignment: .leading
            )
            .clipped()
            .opacity(isCollapsed ? 0 : 1)
    }
}

/// Geometry-affecting snapshot of a tab. Changes to these fields (count, order,
/// selection, pin state) drive an animated refresh; title-only changes do not.
/// Deliberately omits `title`: programs rewrite titles rapidly, and a title
/// change does not move anything in a fixed-width tab.
struct TabStructureSignature: Equatable {
    let id: ObjectIdentifier
    let isSelected: Bool
    let isPinned: Bool
}

/// Plays a one-shot expand-and-fade entrance for a freshly opened tab (or the
/// tab bar itself). Each native tab is a separate window with its own chrome,
/// so opening a tab presents a freshly mounted chrome where ForEach insertion
/// transitions never fire — instead `NativeTabWindowManager` marks the new
/// tab and this container starts collapsed and springs open on first appear.
private struct NativeTabEntrance<Content: View>: View {
    private let collapses: Axis
    private let alignment: Alignment
    private let content: Content
    @State private var isCollapsed: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(
        animatesEntrance: Bool,
        collapses: Axis,
        alignment: Alignment,
        @ViewBuilder content: () -> Content
    ) {
        self.collapses = collapses
        self.alignment = alignment
        self.content = content()
        _isCollapsed = State(initialValue: animatesEntrance)
    }

    var body: some View {
        content
            .frame(
                width: collapses == .horizontal && isCollapsed ? 0 : nil,
                height: collapses == .vertical && isCollapsed ? 0 : nil,
                alignment: alignment
            )
            .clipped()
            .opacity(isCollapsed ? 0 : 1)
            .onAppear {
                guard isCollapsed else {
                    return
                }
                withAnimation(reduceMotion ? nil : NativeTabChromeView.chromeAnimation) {
                    isCollapsed = false
                }
            }
    }
}

private struct NativeTabRenameRequest: Identifiable {
    var descriptor: NativeTabDescriptor

    var id: ObjectIdentifier {
        descriptor.id
    }

    var title: String {
        descriptor.title
    }
}

private struct RenameNativeTabSheet: View {
    @Binding var title: String
    let initialTitle: String
    let onCancel: () -> Void
    let onSave: () -> Void
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Rename Tab")
                .font(.headline)

            TextField(initialTitle, text: $title)
                .textFieldStyle(.roundedBorder)
                .frame(width: 280)
                .focused($isFocused)
                .onSubmit(onSave)

            HStack {
                Button("Clear Name") {
                    title = ""
                    onSave()
                }
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button("Save", action: onSave)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(18)
        .onAppear {
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: 10_000_000)
                isFocused = true
            }
        }
    }
}

private extension View {
    func modifierFlagsTracking(
        _ flags: Binding<NSEvent.ModifierFlags>,
        monitor: Binding<Any?>
    ) -> some View {
        onAppear {
            flags.wrappedValue = NSEvent.modifierFlags
            guard monitor.wrappedValue == nil else {
                return
            }
            monitor.wrappedValue = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { event in
                flags.wrappedValue = event.modifierFlags
                return event
            }
        }
        .onDisappear {
            if let activeMonitor = monitor.wrappedValue {
                NSEvent.removeMonitor(activeMonitor)
                monitor.wrappedValue = nil
            }
        }
    }
}
