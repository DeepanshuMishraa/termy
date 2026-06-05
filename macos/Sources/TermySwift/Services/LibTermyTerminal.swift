import CTermy
import Foundation

enum LibTermyError: Error, CustomStringConvertible {
    case missingTerminal
    case missingCells

    var description: String {
        switch self {
        case .missingTerminal:
            return "libtermy did not return a terminal handle"
        case .missingCells:
            return "libtermy returned a frame without cells"
        }
    }
}

final class LibTermyTerminal {
    private var handle: OpaquePointer?
    private var configHandle: OpaquePointer?

    let renderConfig: TerminalRenderConfig

    init(
        cols: UInt16 = 96,
        rows: UInt16 = 28,
        loadUserConfig: Bool = true,
        workingDirectoryOverride: String? = nil,
        startupCommand: String? = nil
    ) throws {
        var size = termy_size_default()
        size.cols = cols
        size.rows = rows
        let config = try loadUserConfig ? Self.loadDefaultConfig() : nil
        configHandle = config
        renderConfig = try Self.renderConfig(for: config)
        let workingDirectory: String?
        if let override = Self.normalizedWorkingDirectory(workingDirectoryOverride) {
            workingDirectory = override
        } else {
            workingDirectory = try Self.workingDirectory(for: config)
        }

        var terminal: OpaquePointer?
        let workingDirectoryBytes = workingDirectory.map { Array($0.utf8) } ?? []
        let startupCommandBytes = startupCommand.map { Array($0.utf8) } ?? []
        let status = workingDirectoryBytes.withUnsafeBufferPointer { workingDirectoryBuffer in
            startupCommandBytes.withUnsafeBufferPointer { startupCommandBuffer in
                var options = TermyFfiTerminalOptions(
                    config: config,
                    working_directory_ptr: workingDirectoryBuffer.baseAddress,
                    working_directory_len: workingDirectoryBuffer.count,
                    startup_command_ptr: startupCommandBuffer.baseAddress,
                    startup_command_len: startupCommandBuffer.count,
                    env_vars_ptr: nil,
                    env_vars_len: 0
                )
                return termy_terminal_new_with_options(size, &options, &terminal)
            }
        }
        try TermyFfiBridge.requireOK("termy_terminal_new_with_options", status)

        guard let terminal else {
            throw LibTermyError.missingTerminal
        }
        // The Swift shell polls damage on its own cadence, so plain wakeup
        // events only add queue churn. Metadata events still flow through.
        _ = termy_terminal_set_wakeup_enabled(terminal, false)
        handle = terminal
    }

    deinit {
        if let handle {
            _ = termy_terminal_free(handle)
        }
        if let configHandle {
            _ = termy_config_free(configHandle)
        }
    }

    func write(_ bytes: [UInt8]) throws {
        let handle = try terminalHandle()
        let status = bytes.withUnsafeBufferPointer { buffer in
            termy_terminal_write(handle, buffer.baseAddress, buffer.count)
        }
        try TermyFfiBridge.requireOK("termy_terminal_write", status)
    }

    func encodeKey(_ keyInput: TerminalKeyInput) throws -> [UInt8]? {
        let handle = try terminalHandle()

        let keyBytes = Array(keyInput.key.utf8)
        let keyCharBytes = keyInput.keyChar.map { Array($0.utf8) } ?? []
        var outBytes = TermyFfiBytes()

        let status = keyBytes.withUnsafeBufferPointer { keyBuffer in
            keyCharBytes.withUnsafeBufferPointer { keyCharBuffer in
                var ffiKeystroke = TermyFfiKeystroke(
                    control: keyInput.control,
                    alt: keyInput.alt,
                    shift: keyInput.shift,
                    platform: keyInput.platform,
                    function: keyInput.function,
                    key_ptr: keyBuffer.baseAddress,
                    key_len: keyBuffer.count,
                    key_char_ptr: keyCharBuffer.baseAddress,
                    key_char_len: keyCharBuffer.count,
                    event_kind: keyInput.eventKind.rawValue
                )
                return termy_terminal_encode_key(handle, &ffiKeystroke, &outBytes)
            }
        }
        try TermyFfiBridge.requireOK("termy_terminal_encode_key", status)
        defer {
            if outBytes.ptr != nil {
                _ = termy_buffer_free(outBytes)
            }
        }

        guard let ptr = outBytes.ptr, outBytes.len > 0 else {
            return nil
        }
        return Array(UnsafeBufferPointer(start: ptr, count: Int(outBytes.len)))
    }

    func encodeMouse(_ mouseInput: TerminalMouseInput) throws -> [UInt8]? {
        let handle = try terminalHandle()
        var ffiInput = TermyFfiMouseInput(
            kind: mouseInput.kind.rawValue,
            button: mouseInput.button.rawValue,
            col: mouseInput.position.col,
            row: mouseInput.position.row,
            control: mouseInput.control,
            alt: mouseInput.alt,
            shift: mouseInput.shift
        )
        var outBytes = TermyFfiBytes()
        try TermyFfiBridge.requireOK(
            "termy_terminal_encode_mouse",
            termy_terminal_encode_mouse(handle, &ffiInput, &outBytes)
        )
        defer {
            if outBytes.ptr != nil {
                _ = termy_buffer_free(outBytes)
            }
        }

        guard let ptr = outBytes.ptr, outBytes.len > 0 else {
            return nil
        }
        return Array(UnsafeBufferPointer(start: ptr, count: Int(outBytes.len)))
    }

    func resize(cols: UInt16, rows: UInt16, cellWidth: Float, cellHeight: Float) throws {
        let handle = try terminalHandle()
        var size = termy_size_default()
        size.cols = cols
        size.rows = rows
        size.cell_width = cellWidth
        size.cell_height = cellHeight
        try TermyFfiBridge.requireOK("termy_terminal_resize", termy_terminal_resize(handle, size))
    }

    func scrollDisplay(deltaLines: Int32) throws -> Bool {
        try changedBy("termy_terminal_scroll_display") { handle, changed in
            termy_terminal_scroll_display(handle, deltaLines, changed)
        }
    }

    func scrollToBottom() throws -> Bool {
        try changedBy("termy_terminal_scroll_to_bottom") { handle, changed in
            termy_terminal_scroll_to_bottom(handle, changed)
        }
    }

    /// Reload the terminal's theme palette from the on-disk config so existing
    /// cells recolor on the next snapshot.
    func reloadColors() throws {
        let handle = try terminalHandle()
        try TermyFfiBridge.requireOK(
            "termy_terminal_reload_default_config_colors",
            termy_terminal_reload_default_config_colors(handle)
        )
    }

    /// Load a fresh render config (font, metrics, colors, padding) from the
    /// on-disk config without touching any running terminal.
    static func loadRenderConfig() throws -> TerminalRenderConfig {
        let config = try loadDefaultConfig()
        defer {
            if let config {
                _ = termy_config_free(config)
            }
        }
        return try renderConfig(for: config)
    }

    func clearScrollback() throws -> Bool {
        try changedBy("termy_terminal_clear_scrollback") { handle, changed in
            termy_terminal_clear_scrollback(handle, changed)
        }
    }

    func drainEvents() throws -> [TerminalRuntimeEvent] {
        let handle = try terminalHandle()
        var batch = TermyFfiEventBatch()
        try TermyFfiBridge.requireOK(
            "termy_terminal_drain_events",
            termy_terminal_drain_events(handle, &batch)
        )
        defer {
            _ = termy_event_batch_free(&batch)
        }

        guard let eventsPtr = batch.events_ptr else {
            return []
        }
        return UnsafeBufferPointer(start: eventsPtr, count: Int(batch.events_len))
            .compactMap(Self.event(from:))
    }

    /// Polls the core for what changed since the last call. The FFI reports a
    /// `kind` (0 = none, 1 = full, 2 = partial) plus a span list for partial
    /// damage; we surface all of it so the render-plan cache can rebuild only the
    /// rows that actually changed.
    func takeDamage() throws -> TerminalDamage {
        let handle = try terminalHandle()

        var damage = TermyFfiDamage()
        try TermyFfiBridge.requireOK(
            "termy_terminal_take_damage",
            termy_terminal_take_damage(handle, &damage)
        )
        defer { _ = termy_damage_free(&damage) }

        switch damage.kind {
        case 1:
            return .full
        case 2:
            guard let spansPtr = damage.spans_ptr, damage.spans_len > 0 else {
                return .none
            }
            let spans = UnsafeBufferPointer(start: spansPtr, count: damage.spans_len)
                .map {
                    TerminalDirtySpan(
                        row: Int($0.row),
                        leftCol: Int($0.left_col),
                        rightCol: Int($0.right_col)
                    )
                }
            return spans.isEmpty ? .none : .partial(spans)
        default:
            return .none
        }
    }

    func snapshot() throws -> TerminalFrame {
        let handle = try terminalHandle()

        var frame = TermyFfiFrame()
        try TermyFfiBridge.requireOK("termy_terminal_snapshot", termy_terminal_snapshot(handle, &frame))
        defer {
            _ = termy_frame_free(&frame)
        }

        guard let cellsPtr = frame.cells_ptr else {
            throw LibTermyError.missingCells
        }

        let cells = UnsafeBufferPointer(start: cellsPtr, count: Int(frame.cells_len))
            .map(Self.cell(from:))
        let cursor = frame.cursor.visible
            ? TerminalCursor(
                col: Int(frame.cursor.col),
                row: Int(frame.cursor.row),
                style: TerminalCursorStyle(ffiRawValue: frame.cursor.style)
            )
            : nil

        return TerminalFrame(
            cols: Int(frame.cols),
            rows: Int(frame.rows),
            cells: cells,
            cursor: cursor,
            displayOffset: Int(frame.display_offset),
            historySize: Int(frame.history_size)
        )
    }

    func frameUpdate(forceFull: Bool) throws -> TerminalFrameUpdate {
        let handle = try terminalHandle()

        var update = TermyFfiFrameUpdate()
        try TermyFfiBridge.requireOK(
            "termy_terminal_take_frame_update",
            termy_terminal_take_frame_update(handle, forceFull, &update)
        )
        defer {
            _ = termy_frame_update_free(&update)
        }

        let cells: [TerminalCell]
        if update.cells_len > 0 {
            guard let cellsPtr = update.cells_ptr else {
                throw LibTermyError.missingCells
            }
            cells = UnsafeBufferPointer(start: cellsPtr, count: Int(update.cells_len))
                .map(Self.cell(from:))
        } else {
            cells = []
        }

        let damage: TerminalDamage
        switch update.damage_kind {
        case 1:
            damage = .full
        case 2:
            if update.spans_len > 0 {
                guard let spansPtr = update.spans_ptr else {
                    damage = .none
                    break
                }
                let spans = UnsafeBufferPointer(start: spansPtr, count: Int(update.spans_len))
                    .map {
                        TerminalDirtySpan(
                            row: Int($0.row),
                            leftCol: Int($0.left_col),
                            rightCol: Int($0.right_col)
                        )
                    }
                damage = .partial(spans)
            } else {
                damage = .none
            }
        default:
            damage = .none
        }

        return TerminalFrameUpdate(
            cols: Int(update.cols),
            rows: Int(update.rows),
            cells: cells,
            cursor: Self.cursor(from: update.cursor),
            displayOffset: Int(update.display_offset),
            historySize: Int(update.history_size),
            damage: damage
        )
    }

    /// The OSC 8 hyperlink under a viewport cell, if any, expanded to the
    /// contiguous link run on that row. Best-effort: returns nil on FFI errors
    /// since hover lookups should never surface failures.
    func hyperlink(atRow row: Int, col: Int) -> TerminalFrameLink? {
        guard row >= 0, col >= 0, let handle else {
            return nil
        }

        var found = false
        var link = TermyFfiHyperlink()
        guard termy_terminal_hyperlink_at(handle, row, col, &found, &link) == TERMY_FFI_OK,
              found
        else {
            return nil
        }
        defer { _ = termy_hyperlink_free(&link) }

        guard let target = TermyFfiBridge.string(from: link.uri), !target.isEmpty else {
            return nil
        }
        return TerminalFrameLink(
            row: row,
            startCol: Int(link.start_col),
            endCol: Int(link.end_col),
            target: target
        )
    }

    func search(
        _ query: String,
        options: TerminalSearchOptions = TerminalSearchOptions()
    ) throws -> [TerminalSearchMatch] {
        let handle = try terminalHandle()

        var batch = TermyFfiSearchBatch()
        let ffiOptions = TermyFfiSearchOptions(
            case_sensitive: options.caseSensitive,
            regex: options.usesRegex
        )
        let status = Array(query.utf8).withUnsafeBufferPointer { buffer in
            termy_terminal_search_with_options(
                handle,
                buffer.baseAddress,
                buffer.count,
                ffiOptions,
                &batch
            )
        }
        try TermyFfiBridge.requireOK("termy_terminal_search_with_options", status)
        defer {
            _ = termy_search_batch_free(&batch)
        }

        guard let matchesPtr = batch.matches_ptr else {
            return []
        }
        return UnsafeBufferPointer(start: matchesPtr, count: Int(batch.matches_len))
            .map { match in
                TerminalSearchMatch(
                    row: Int(match.row),
                    startCol: Int(match.start_col),
                    endCol: Int(match.end_col)
                )
            }
    }

    private func terminalHandle() throws -> OpaquePointer {
        guard let handle else {
            throw LibTermyError.missingTerminal
        }
        return handle
    }

    private func changedBy(
        _ operation: String,
        _ call: (OpaquePointer, UnsafeMutablePointer<Bool>) -> TermyFfiStatus
    ) throws -> Bool {
        let handle = try terminalHandle()
        var changed = false
        try TermyFfiBridge.requireOK(operation, call(handle, &changed))
        return changed
    }

    private static func loadDefaultConfig() throws -> OpaquePointer? {
        var config: OpaquePointer?
        try TermyFfiBridge.requireOK("termy_config_load_default", termy_config_load_default(&config))
        return config
    }

    private static func renderConfig(for config: OpaquePointer?) throws -> TerminalRenderConfig {
        guard let config else {
            return .default
        }

        var renderConfig = TermyFfiRenderConfig()
        try TermyFfiBridge.requireOK(
            "termy_config_render_config_for_appearance",
            termy_config_render_config_for_appearance(
                config,
                Self.currentSystemAppearanceRawValue(),
                &renderConfig
            )
        )
        defer {
            _ = termy_render_config_free(&renderConfig)
        }
        return TerminalRenderConfig(renderConfig)
    }

    private static func currentSystemAppearanceRawValue() -> UInt32 {
        UserDefaults.standard.string(forKey: "AppleInterfaceStyle") == "Dark" ? 1 : 0
    }

    private static func workingDirectory(for config: OpaquePointer?) throws -> String? {
        guard let config else {
            return nil
        }

        var bytes = TermyFfiBytes()
        try TermyFfiBridge.requireOK(
            "termy_config_working_directory",
            termy_config_working_directory(config, &bytes)
        )
        defer {
            if bytes.ptr != nil {
                _ = termy_buffer_free(bytes)
            }
        }

        guard bytes.ptr != nil, bytes.len > 0 else {
            return nil
        }
        let value = TermyFfiBridge.string(from: bytes, trimmingWhitespaceAndNewlines: true) ?? ""
        return value.isEmpty ? nil : value
    }

    private static func normalizedWorkingDirectory(_ value: String?) -> String? {
        guard let value else {
            return nil
        }

        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private static func event(from event: TermyFfiEvent) -> TerminalRuntimeEvent? {
        guard let eventKind = TerminalRuntimeEventKind(rawValue: event.kind) else {
            return nil
        }

        switch eventKind {
        case .wakeup:
            return .wakeup
        case .title:
            return .title(TermyFfiBridge.string(from: event.payload) ?? "")
        case .resetTitle:
            return .resetTitle
        case .bell:
            return .bell
        case .exit:
            return .exit
        case .clipboardStore:
            return .clipboardStore(TermyFfiBridge.string(from: event.payload) ?? "")
        case .shellPromptStart:
            return .shellPromptStart
        case .shellCommandStart:
            return .shellCommandStart
        case .shellCommandExecuting:
            return .shellCommandExecuting
        case .shellCommandFinished:
            return .shellCommandFinished(event.exit_code >= 0 ? event.exit_code : nil)
        case .progress:
            return .progress(TerminalProgress(
                state: event.progress_state,
                value: event.progress_value
            ))
        case .workingDirectory:
            return .workingDirectory(TermyFfiBridge.string(from: event.payload) ?? "")
        }
    }

    private static func cell(from ffiCell: TermyFfiCell) -> TerminalCell {
        TerminalCell(
            col: Int(ffiCell.col),
            row: Int(ffiCell.row),
            character: character(from: ffiCell.codepoint),
            foreground: TerminalRGBA(ffiCell.fg),
            background: TerminalRGBA(ffiCell.bg),
            usesTerminalDefaultBackground: ffiCell.uses_terminal_default_bg,
            renderText: ffiCell.render_text,
            bold: ffiCell.bold
        )
    }

    private static func cursor(from ffiCursor: TermyFfiCursor) -> TerminalCursor? {
        guard ffiCursor.visible else {
            return nil
        }
        return TerminalCursor(
            col: Int(ffiCursor.col),
            row: Int(ffiCursor.row),
            style: TerminalCursorStyle(ffiRawValue: ffiCursor.style)
        )
    }

    private static func character(from codepoint: UInt32) -> Character {
        UnicodeScalar(codepoint).map(Character.init) ?? " "
    }

}
