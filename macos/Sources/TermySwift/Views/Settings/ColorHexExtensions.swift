import SwiftUI

extension Color {
    init?(hex: String) {
        guard let rgb = RGBHexColor(hex: hex) else {
            return nil
        }
        self = Color(
            red: rgb.red,
            green: rgb.green,
            blue: rgb.blue
        )
    }

    var hexString: String? {
        guard let srgb = NSColor(self).usingColorSpace(.sRGB) else {
            return nil
        }
        let r = Int((srgb.redComponent * 255).rounded())
        let g = Int((srgb.greenComponent * 255).rounded())
        let b = Int((srgb.blueComponent * 255).rounded())
        return String(format: "#%02x%02x%02x", r, g, b)
    }
}

struct RGBHexColor {
    var red: Double
    var green: Double
    var blue: Double

    init?(hex: String) {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else {
            return nil
        }
        if value.hasPrefix("#") {
            value.removeFirst()
        }
        guard value.count == 6, let packed = Int(value, radix: 16) else {
            return nil
        }
        red = Self.component(packed >> 16)
        green = Self.component(packed >> 8)
        blue = Self.component(packed)
    }

    private static func component(_ packedComponent: Int) -> Double {
        Double(packedComponent & 0xff) / 255.0
    }
}
