import SwiftUI

/// Bottom-trailing stack of transient toasts. Tap to dismiss early.
struct TermyToastOverlay: View {
    @ObservedObject private var center = TermyToastCenter.shared

    var body: some View {
        VStack(alignment: .trailing, spacing: 8) {
            ForEach(center.toasts) { toast in
                HStack(spacing: 8) {
                    Image(systemName: icon(for: toast.kind))
                        .foregroundStyle(color(for: toast.kind))
                    Text(toast.message)
                        .font(.system(size: 12))
                        .lineLimit(2)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(.regularMaterial, in: Capsule())
                .overlay(Capsule().strokeBorder(.separator, lineWidth: 0.5))
                .contentShape(Capsule())
                .onTapGesture {
                    center.dismiss(toast.id)
                }
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        .allowsHitTesting(!center.toasts.isEmpty)
        .animation(.spring(response: 0.3, dampingFraction: 0.85), value: center.toasts)
    }

    private func icon(for kind: TermyToast.Kind) -> String {
        switch kind {
        case .info: return "info.circle"
        case .success: return "checkmark.circle"
        case .warning: return "exclamationmark.triangle"
        }
    }

    private func color(for kind: TermyToast.Kind) -> Color {
        switch kind {
        case .info: return .secondary
        case .success: return .green
        case .warning: return .orange
        }
    }
}
