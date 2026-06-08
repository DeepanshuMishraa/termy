import AppKit

/// Surfaces a failed user action as a modal alert. Used where an error must not
/// be swallowed but there is no inline UI channel to route it through.
@MainActor
enum TermyErrorPresenter {
    static func present(_ title: String, message: String) {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = title
        alert.informativeText = message
        alert.runModal()
    }

    static func present(_ title: String, error: Error) {
        present(title, message: String(describing: error))
    }
}
