import SwiftUI

struct ColorSettingsContent: View {
    let colors: [ColorSetting]
    @ObservedObject var store: SettingsStore

    var body: some View {
        Section("Base") {
            ForEach(colors.prefix(3)) { color in
                ColorRow(color: color, store: store)
            }
        }
        Section("ANSI Palette") {
            ForEach(colors.dropFirst(3)) { color in
                ColorRow(color: color, store: store)
            }
        }
    }
}

struct ColorRow: View {
    let color: ColorSetting
    @ObservedObject var store: SettingsStore

    var body: some View {
        LabeledContent {
            HStack(spacing: 10) {
                ColorPicker("", selection: pickerBinding, supportsOpacity: false)
                    .labelsHidden()
                if !store.colorHex(for: color.key).isEmpty {
                    Button("Reset") {
                        store.commitColor(key: color.key, hex: nil)
                    }
                    .buttonStyle(.borderless)
                    .font(.caption)
                } else {
                    Text("theme")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        } label: {
            SettingLabelView(color: color)
        }
    }

    private var pickerBinding: Binding<Color> {
        Binding(
            get: { Color(hex: store.colorHex(for: color.key)) ?? Color(white: 0.5) },
            set: { newColor in
                if let hex = newColor.hexString {
                    store.commitColor(key: color.key, hex: hex)
                }
            }
        )
    }
}
