import SwiftUI

struct SettingRow: View {
    let setting: Setting
    @ObservedObject var store: SettingsStore

    var body: some View {
        switch setting.kind {
        case .boolean:
            HStack {
                Toggle(isOn: store.boolBinding(setting.key)) {
                    SettingLabelView(setting: setting)
                }
                Spacer()
                SettingResetButton(key: setting.key, store: store)
            }
        case .enumeration:
            ChoiceSettingRow(setting: setting, store: store)
        case .numeric:
            NumericSettingRow(setting: setting, store: store)
        case .text:
            CommittingTextFieldRow(setting: setting, store: store, maxWidth: 240)
        case .special:
            if setting.choices?.isEmpty == false {
                ChoiceSettingRow(setting: setting, store: store)
            } else {
                CommittingTextFieldRow(setting: setting, store: store, maxWidth: 240)
            }
        }
    }
}

struct ChoiceSettingRow: View {
    let setting: Setting
    @ObservedObject var store: SettingsStore

    var body: some View {
        SettingLabeledContent(setting: setting) {
            HStack(spacing: 6) {
                Picker(selection: store.enumBinding(setting.key)) {
                    ForEach(setting.choices ?? []) { choice in
                        Text(choice.label).tag(choice.value)
                    }
                } label: {
                    EmptyView()
                }
                SettingResetButton(key: setting.key, store: store)
            }
        }
    }
}

struct SettingLabeledContent<Content: View>: View {
    let setting: Setting
    @ViewBuilder var content: Content

    var body: some View {
        LabeledContent {
            content
        } label: {
            SettingLabelView(setting: setting)
        }
    }
}

struct SettingLabelView: View {
    let title: String
    let description: String

    init(setting: Setting) {
        title = setting.title
        description = setting.description
    }

    init(color: ColorSetting) {
        title = color.title
        description = color.description
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            Text(description)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

struct NumericSettingRow: View {
    let setting: Setting
    @ObservedObject var store: SettingsStore

    var body: some View {
        if let range = Self.sliderRange(for: setting.key) {
            SettingLabeledContent(setting: setting) {
                HStack(spacing: 10) {
                    Slider(
                        value: Binding(
                            get: { Double(store.value(for: setting.key)) ?? range.lowerBound },
                            set: { store.commitRoot(key: setting.key, value: Self.format($0)) }
                        ),
                        in: range,
                        step: Self.step(for: setting.key)
                    )
                    .frame(width: 180)
                    Text(store.value(for: setting.key))
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .frame(width: 48, alignment: .trailing)
                    SettingResetButton(key: setting.key, store: store)
                }
            }
        } else {
            HStack {
                CommittingTextFieldRow(setting: setting, store: store, maxWidth: 120)
                SettingResetButton(key: setting.key, store: store)
            }
        }
    }

    private static func sliderRange(for key: String) -> ClosedRange<Double>? {
        switch key {
        case "background_opacity":
            return 0...1
        case "pane_focus_strength":
            return 0...2
        case "line_height":
            return 0.8...2.5
        case "mouse_scroll_multiplier":
            return 0.1...10
        default:
            return nil
        }
    }

    private static func step(for key: String) -> Double {
        key == "mouse_scroll_multiplier" ? 0.1 : 0.05
    }

    private static func format(_ value: Double) -> String {
        String(format: "%.2f", value)
    }
}

struct CommittingTextFieldRow: View {
    let setting: Setting
    @ObservedObject var store: SettingsStore
    let maxWidth: CGFloat

    @State private var text: String = ""
    @FocusState private var focused: Bool

    var body: some View {
        SettingLabeledContent(setting: setting) {
            HStack(spacing: 6) {
                TextField(setting.title, text: $text)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: maxWidth)
                    .focused($focused)
                    .onSubmit(commit)
                    .onChange(of: focused) { _, isFocused in
                        if !isFocused {
                            commit()
                        }
                    }
                SettingResetButton(key: setting.key, store: store)
            }
        }
        .onAppear {
            text = store.value(for: setting.key)
        }
        .onChange(of: store.value(for: setting.key)) { _, newValue in
            if !focused {
                text = newValue
            }
        }
    }

    private func commit() {
        store.commitRoot(key: setting.key, value: text)
    }
}

struct SettingResetButton: View {
    let key: String
    @ObservedObject var store: SettingsStore

    var body: some View {
        Button {
            store.resetSetting(key: key)
        } label: {
            Image(systemName: "arrow.uturn.backward")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.borderless)
        .help("Reset to default")
        .opacity(0.6)
    }
}
