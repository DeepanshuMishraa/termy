import CTermy
import Foundation

/// A parsed tmux control-mode notification, mirroring the FFI
/// `TermyFfiTmuxNotification` kinds.
enum TmuxControlNotification: Equatable {
    case output(paneID: String, bytes: [UInt8])
    case needsRefresh
    case warning(String)
    case exit(String?)
}

/// Swift wrapper over the `termy_tmux_control_*` FFI: drives a `tmux -CC`
/// control session and surfaces parsed notifications. The native
/// `TmuxControlSession` (pane mapping/rendering) builds on this.
final class LibTermyTmuxControl {
    enum ControlError: Error, CustomStringConvertible {
        case launchFailed
        case closed

        var description: String {
            switch self {
            case .launchFailed: return "failed to launch tmux control session"
            case .closed: return "tmux control session is closed"
            }
        }
    }

    private var handle: OpaquePointer?

    init(binary: String, socket: String, session: String) throws {
        let binaryBytes = Array(binary.utf8)
        let socketBytes = Array(socket.utf8)
        let sessionBytes = Array(session.utf8)
        var handle: OpaquePointer?
        let status = binaryBytes.withUnsafeBufferPointer { binaryBuffer in
            socketBytes.withUnsafeBufferPointer { socketBuffer in
                sessionBytes.withUnsafeBufferPointer { sessionBuffer in
                    termy_tmux_control_open(
                        binaryBuffer.baseAddress,
                        binaryBuffer.count,
                        socketBuffer.baseAddress,
                        socketBuffer.count,
                        sessionBuffer.baseAddress,
                        sessionBuffer.count,
                        &handle
                    )
                }
            }
        }
        guard status == TERMY_FFI_OK, let handle else {
            throw ControlError.launchFailed
        }
        self.handle = handle
    }

    /// Non-blocking drain of pending control notifications.
    func poll() -> [TmuxControlNotification] {
        guard let handle else {
            return []
        }
        var batch = TermyFfiTmuxNotificationBatch()
        guard termy_tmux_control_poll(handle, &batch) == TERMY_FFI_OK else {
            return []
        }
        defer { _ = termy_tmux_control_notifications_free(&batch) }
        guard let ptr = batch.notifications_ptr, batch.notifications_len > 0 else {
            return []
        }
        return UnsafeBufferPointer(start: ptr, count: batch.notifications_len).map(Self.decode)
    }

    /// Runs a tmux command over the control channel and returns its output.
    @discardableResult
    func send(_ command: String) throws -> String {
        guard let handle else {
            throw ControlError.closed
        }
        var output = TermyFfiBytes()
        let status = Array(command.utf8).withUnsafeBufferPointer { buffer in
            termy_tmux_control_send(handle, buffer.baseAddress, buffer.count, &output)
        }
        defer {
            if output.ptr != nil {
                _ = termy_buffer_free(output)
            }
        }
        try TermyFfiBridge.requireOK("termy_tmux_control_send", status)
        return TermyFfiBridge.string(from: output) ?? ""
    }

    deinit {
        if let handle {
            termy_tmux_control_close(handle)
        }
    }

    private static func decode(_ notification: TermyFfiTmuxNotification) -> TmuxControlNotification {
        switch notification.kind {
        case 0:
            let paneID = TermyFfiBridge.string(from: notification.pane_id) ?? ""
            var bytes: [UInt8] = []
            if let ptr = notification.data.ptr, notification.data.len > 0 {
                bytes = Array(UnsafeBufferPointer(start: ptr, count: Int(notification.data.len)))
            }
            return .output(paneID: paneID, bytes: bytes)
        case 2:
            return .warning(TermyFfiBridge.string(from: notification.data) ?? "")
        case 3:
            let message = TermyFfiBridge.string(from: notification.data) ?? ""
            return .exit(message.isEmpty ? nil : message)
        default:
            return .needsRefresh
        }
    }
}
