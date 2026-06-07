import AppKit
import SwiftUI

struct NativeTabChromeView: View {
    weak var window: NSWindow?
    let configuration: TermyAppConfiguration

    @State private var hoveredTabID: ObjectIdentifier?
    @State private var modifierFlags: NSEvent.ModifierFlags = []
    @State private var modifierMonitor: Any?
    @State private var refreshToken = 0
    @State private var renameRequest: NativeTabRenameRequest?
    @State private var renameText = ""

    private var tabs: [NativeTabDescriptor] {
        _ = refreshToken
        return NativeTabWindowManager.shared.tabDescriptors(for: window)
    }

    /// Shared spring for tab entrances, removals, and width changes —
    /// approximates the system tab bar's feel.
    static let chromeAnimation: Animation = .spring(response: 0.3, dampingFraction: 0.85)

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
        .background(.bar)
        .overlay(alignment: .bottom) {
            Divider()
        }
        .modifierFlagsTracking($modifierFlags, monitor: $modifierMonitor)
        .onReceive(NotificationCenter.default.publisher(for: .termyNativeTabsChanged)) { _ in
            withAnimation(Self.chromeAnimation) {
                refreshToken &+= 1
            }
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
        }
        .frame(width: 164)
        .background(.bar)
        .overlay(alignment: .leading) {
            Divider()
        }
        .modifierFlagsTracking($modifierFlags, monitor: $modifierMonitor)
        .onReceive(NotificationCenter.default.publisher(for: .termyNativeTabsChanged)) { _ in
            withAnimation(Self.chromeAnimation) {
                refreshToken &+= 1
            }
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
        .background(tab.isSelected ? Color.accentColor.opacity(0.16) : Color.clear, in: RoundedRectangle(cornerRadius: 6))
        .contentShape(Rectangle())
        .onHover { hovering in
            hoveredTabID = hovering ? tab.id : nil
        }
        .onTapGesture {
            NativeTabWindowManager.shared.selectNativeTab(tab)
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

    private func beginRename(_ tab: NativeTabDescriptor) {
        renameText = tab.title
        renameRequest = NativeTabRenameRequest(descriptor: tab)
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
                withAnimation(NativeTabChromeView.chromeAnimation) {
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
