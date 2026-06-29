import AppKit
import SwiftUI

struct KeybindSettingsContent: View {
    @ObservedObject var store: SettingsStore
    @ObservedObject private var configurationStore = TermyConfigurationStore.shared
    @State private var search = ""
    @State private var capturingTrigger: String?
    @State private var captureMonitor: Any?
    @State private var showAddSheet = false
    @State private var showRawEditor = false

    private var bindings: [TermyKeybindConfiguration] {
        let all = configurationStore.configuration.keybinds
            .sorted { $0.action < $1.action }
        let query = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !query.isEmpty else {
            return all
        }
        return all.filter {
            $0.action.lowercased().contains(query) || $0.trigger.lowercased().contains(query)
        }
    }

    private var conflictingTriggers: Set<String> {
        TerminalKeybindConflicts.conflictingTriggers(in: configurationStore.configuration.keybinds)
    }

    var body: some View {
        let conflicts = conflictingTriggers

        Section("Active Keybindings") {
            TextField("Filter actions or keys", text: $search)
                .textFieldStyle(.roundedBorder)

            if !conflicts.isEmpty {
                Label(
                    "\(conflicts.count) trigger\(conflicts.count == 1 ? "" : "s") bound more than once — only the first binding takes effect.",
                    systemImage: "exclamationmark.triangle.fill"
                )
                .font(.caption)
                .foregroundStyle(.orange)
            }

            if bindings.isEmpty {
                Text("No keybindings match.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(bindings, id: \.self) { binding in
                    KeybindRow(
                        binding: binding,
                        isConflicting: conflicts.contains(binding.trigger),
                        isCapturing: capturingTrigger == binding.trigger,
                        onRekey: { startCapture(for: binding) },
                        onDelete: {
                            store.deleteKeybind(trigger: binding.trigger, action: binding.action)
                        }
                    )
                }
            }

            Button {
                showAddSheet = true
            } label: {
                Label("Add Keybinding", systemImage: "plus")
            }
            .buttonStyle(.borderless)
        }

        Section("Advanced") {
            DisclosureGroup("Raw Directives", isExpanded: $showRawEditor) {
                Text("One directive per line, e.g. `cmd-k=clear_buffer`.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                TextEditor(text: $store.keybindsText)
                    .font(.system(.body, design: .monospaced))
                    .frame(minHeight: 180)
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(Color.secondary.opacity(0.25), lineWidth: 1)
                    )

                HStack {
                    Spacer()
                    Button("Apply Raw Keybinds") {
                        store.commitKeybinds()
                    }
                    .keyboardShortcut("s", modifiers: [.command])
                }
            }
        }
        .sheet(isPresented: $showAddSheet) {
            AddKeybindSheet(
                store: store,
                existingTriggers: Set(configurationStore.configuration.keybinds.map(\.trigger)),
                onDismiss: { showAddSheet = false }
            )
        }
        .onDisappear {
            stopCapture()
        }
    }

    private func startCapture(for binding: TermyKeybindConfiguration) {
        stopCapture()
        capturingTrigger = binding.trigger
        captureMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == 53 {
                Task { @MainActor in stopCapture() }
                return nil
            }
            let triggers = NativeKeyEventClassifier.canonicalTriggers(for: event)
            if let trigger = triggers.first(where: { !$0.contains("secondary") }) ?? triggers.first {
                let action = binding.action
                let oldTrigger = binding.trigger
                Task { @MainActor in
                    store.updateKeybindTrigger(
                        action: action,
                        oldTrigger: oldTrigger,
                        newTrigger: trigger
                    )
                    stopCapture()
                }
            }
            return nil
        }
    }

    private func stopCapture() {
        if let monitor = captureMonitor {
            NSEvent.removeMonitor(monitor)
            captureMonitor = nil
        }
        capturingTrigger = nil
    }
}

private struct KeybindRow: View {
    let binding: TermyKeybindConfiguration
    let isConflicting: Bool
    let isCapturing: Bool
    let onRekey: () -> Void
    let onDelete: () -> Void

    var body: some View {
        LabeledContent {
            HStack(spacing: 6) {
                if isConflicting {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                        .help("This trigger is bound more than once.")
                }
                Button(action: onRekey) {
                    Text(isCapturing ? "Press a key…" : binding.trigger)
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(isCapturing ? Color.white : Color.secondary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(
                            RoundedRectangle(cornerRadius: 5)
                                .fill(isCapturing ? Color.accentColor : Color.secondary.opacity(0.12))
                        )
                }
                .buttonStyle(.plain)
                .help("Click to rebind")

                Button(action: onDelete) {
                    Image(systemName: "minus.circle.fill")
                        .foregroundStyle(.red.opacity(0.7))
                }
                .buttonStyle(.borderless)
                .help("Remove binding")
            }
        } label: {
            Text(KeybindSettingsContent.humanize(binding.action))
        }
    }
}

private struct AddKeybindSheet: View {
    @ObservedObject var store: SettingsStore
    let existingTriggers: Set<String>
    let onDismiss: () -> Void

    @State private var selectedAction = "new_tab"
    @State private var capturedTrigger: String?
    @State private var isCapturing = false
    @State private var captureMonitor: Any?
    @State private var conflictError: String?

    var body: some View {
        VStack(spacing: 16) {
            Text("Add Keybinding")
                .font(.headline)

            Picker("Action", selection: $selectedAction) {
                ForEach(KeybindSettingsContent.bindableActions, id: \.identifier) { action in
                    Text(action.label).tag(action.identifier)
                }
            }

            HStack {
                if let trigger = capturedTrigger {
                    Text(trigger)
                        .font(.system(.body, design: .monospaced))
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(RoundedRectangle(cornerRadius: 5).fill(Color.secondary.opacity(0.12)))
                }
                Button(isCapturing ? "Press a key…" : "Record Key") {
                    if isCapturing {
                        stopCapture()
                    } else {
                        startCapture()
                    }
                }
                .buttonStyle(.bordered)
            }

            if let conflictError {
                Text(conflictError)
                    .font(.caption)
                    .foregroundStyle(.orange)
            }

            HStack {
                Button("Cancel") {
                    stopCapture()
                    onDismiss()
                }
                .keyboardShortcut(.escape, modifiers: [])

                Spacer()

                Button("Add") {
                    guard let trigger = capturedTrigger else { return }
                    if existingTriggers.contains(trigger) {
                        conflictError = "Trigger \(trigger) is already bound. Remove or rebind the existing one first."
                        return
                    }
                    store.addKeybind(trigger: trigger, action: selectedAction)
                    stopCapture()
                    onDismiss()
                }
                .keyboardShortcut(.return, modifiers: [])
                .disabled(capturedTrigger == nil)
            }
        }
        .padding(20)
        .frame(width: 380)
        .onDisappear {
            stopCapture()
        }
    }

    private func startCapture() {
        stopCapture()
        isCapturing = true
        conflictError = nil
        captureMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == 53 {
                Task { @MainActor in stopCapture() }
                return nil
            }
            let triggers = NativeKeyEventClassifier.canonicalTriggers(for: event)
            if let trigger = triggers.first(where: { !$0.contains("secondary") }) ?? triggers.first {
                Task { @MainActor in
                    capturedTrigger = trigger
                    stopCapture()
                }
            }
            return nil
        }
    }

    private func stopCapture() {
        if let monitor = captureMonitor {
            NSEvent.removeMonitor(monitor)
            captureMonitor = nil
        }
        isCapturing = false
    }
}

extension KeybindSettingsContent {
    static let bindableActions: [(identifier: String, label: String)] = [
        ("new_tab", "New Tab"),
        ("close_tab", "Close Tab"),
        ("close_pane_or_tab", "Close Pane or Tab"),
        ("close_pane", "Close Pane"),
        ("split_pane_vertical", "Split Right"),
        ("split_pane_horizontal", "Split Down"),
        ("focus_pane_next", "Next Pane"),
        ("focus_pane_previous", "Previous Pane"),
        ("focus_pane_left", "Focus Pane Left"),
        ("focus_pane_right", "Focus Pane Right"),
        ("focus_pane_up", "Focus Pane Up"),
        ("focus_pane_down", "Focus Pane Down"),
        ("resize_pane_left", "Resize Pane Left"),
        ("resize_pane_right", "Resize Pane Right"),
        ("resize_pane_up", "Resize Pane Up"),
        ("resize_pane_down", "Resize Pane Down"),
        ("toggle_pane_zoom", "Toggle Pane Zoom"),
        ("increase_font_size", "Increase Font Size"),
        ("decrease_font_size", "Decrease Font Size"),
        ("reset_font_size", "Reset Font Size"),
        ("copy", "Copy"),
        ("paste", "Paste"),
        ("open_search", "Find"),
        ("close_search", "Close Search"),
        ("search_next", "Find Next"),
        ("search_previous", "Find Previous"),
        ("toggle_search_case_sensitive", "Toggle Case Sensitive Search"),
        ("toggle_search_regex", "Toggle Regex Search"),
        ("clear_buffer", "Clear Scrollback"),
        ("send_interrupt", "Send Interrupt"),
        ("toggle_command_palette", "Toggle Command Palette"),
        ("toggle_tab_bar_visibility", "Toggle Tab Bar Visibility"),
        ("move_tab_left", "Move Tab Left"),
        ("move_tab_right", "Move Tab Right"),
        ("switch_tab_left", "Switch Tab Left"),
        ("switch_tab_right", "Switch Tab Right"),
        ("open_config", "Open Config"),
        ("prettify_config", "Prettify Config"),
        ("app_info", "App Info"),
        ("restart_app", "Restart App"),
        ("minimize_window", "Minimize Window"),
        ("quit", "Quit"),
    ]

    static func humanize(_ action: String) -> String {
        action
            .split(separator: "_")
            .map { $0.prefix(1).uppercased() + $0.dropFirst() }
            .joined(separator: " ")
    }
}
