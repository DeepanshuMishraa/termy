import Combine
import Foundation

struct TermyToast: Identifiable, Equatable {
    enum Kind: Equatable {
        case info
        case success
        case warning
    }

    let id: Int
    var kind: Kind
    var message: String
}

/// A small transient-notification queue. Toasts auto-dismiss and the visible set
/// is capped so a burst can't stack indefinitely.
@MainActor
final class TermyToastCenter: ObservableObject {
    static let shared = TermyToastCenter()
    static let maxVisible = 3

    @Published private(set) var toasts: [TermyToast] = []
    private var nextID = 0

    func show(_ message: String, kind: TermyToast.Kind = .info, autoDismiss: Bool = true) {
        let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }
        nextID += 1
        let toast = TermyToast(id: nextID, kind: kind, message: trimmed)
        toasts.append(toast)
        if toasts.count > Self.maxVisible {
            toasts.removeFirst(toasts.count - Self.maxVisible)
        }
        guard autoDismiss else {
            return
        }
        let id = toast.id
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(4))
            dismiss(id)
        }
    }

    func dismiss(_ id: Int) {
        toasts.removeAll { $0.id == id }
    }
}
