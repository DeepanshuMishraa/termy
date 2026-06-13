import CTermy
import Foundation

/// Thin wrapper over the libtermy settings C functions. All writes target the
/// shared config file at `~/.config/termy/config.txt` and preserve comments and
/// formatting (the Rust side does surgical edits).
enum SettingsBridge {
    enum BridgeError: Error, CustomStringConvertible {
        case decode(String)

        var description: String {
            switch self {
            case let .decode(message):
                return message
            }
        }
    }

    static func loadSchema() throws -> SettingsSchema {
        var config: OpaquePointer?
        try TermyFfiBridge.requireOK("termy_config_load_default", termy_config_load_default(&config))
        defer {
            if let config {
                _ = termy_config_free(config)
            }
        }

        var bytes = TermyFfiBytes()
        try TermyFfiBridge.requireOK("termy_settings_schema_json", termy_settings_schema_json(config, &bytes))
        return try decodeSchema(from: bytes)
    }

    static func loadSchema(contents: String) throws -> SettingsSchema {
        var config: OpaquePointer?
        let contentsBytes = Array(contents.utf8)
        let status = contentsBytes.withUnsafeBufferPointer { buffer in
            termy_config_from_contents(buffer.baseAddress, buffer.count, &config)
        }
        try TermyFfiBridge.requireOK("termy_config_from_contents", status)
        defer {
            if let config {
                _ = termy_config_free(config)
            }
        }

        var bytes = TermyFfiBytes()
        try TermyFfiBridge.requireOK("termy_settings_schema_json", termy_settings_schema_json(config, &bytes))
        return try decodeSchema(from: bytes)
    }

    private static func decodeSchema(from bytes: TermyFfiBytes) throws -> SettingsSchema {
        defer {
            if bytes.ptr != nil {
                _ = termy_buffer_free(bytes)
            }
        }

        guard let ptr = bytes.ptr, bytes.len > 0 else {
            throw BridgeError.decode("settings schema was empty")
        }
        let data = Data(bytes: ptr, count: Int(bytes.len))
        return try JSONDecoder().decode(SettingsSchema.self, from: data)
    }

    static func setRoot(key: String, value: String) throws {
        let keyBytes = Array(key.utf8)
        let valueBytes = Array(value.utf8)
        let status = keyBytes.withUnsafeBufferPointer { keyBuffer in
            valueBytes.withUnsafeBufferPointer { valueBuffer in
                termy_settings_set_root(
                    keyBuffer.baseAddress,
                    keyBuffer.count,
                    valueBuffer.baseAddress,
                    valueBuffer.count
                )
            }
        }
        try TermyFfiBridge.requireOK("termy_settings_set_root", status)
    }

    static func resetRoot(key: String) throws {
        let keyBytes = Array(key.utf8)
        let status = keyBytes.withUnsafeBufferPointer { keyBuffer in
            termy_settings_reset_root(keyBuffer.baseAddress, keyBuffer.count)
        }
        try TermyFfiBridge.requireOK("termy_settings_reset_root", status)
    }

    /// Pass `hex == nil` to clear the override and inherit the theme color.
    static func setColor(key: String, hex: String?) throws {
        let keyBytes = Array(key.utf8)
        let hexBytes = hex.map { Array($0.utf8) } ?? []
        let status = keyBytes.withUnsafeBufferPointer { keyBuffer in
            hexBytes.withUnsafeBufferPointer { hexBuffer in
                termy_settings_set_color(
                    keyBuffer.baseAddress,
                    keyBuffer.count,
                    hex == nil ? nil : hexBuffer.baseAddress,
                    hex == nil ? 0 : hexBuffer.count
                )
            }
        }
        try TermyFfiBridge.requireOK("termy_settings_set_color", status)
    }

    static func setKeybinds(_ text: String) throws {
        let textBytes = Array(text.utf8)
        let status = textBytes.withUnsafeBufferPointer { textBuffer in
            termy_settings_set_keybinds(textBuffer.baseAddress, textBuffer.count)
        }
        try TermyFfiBridge.requireOK("termy_settings_set_keybinds", status)
    }

    static func installTheme(slug: String) throws {
        let slugBytes = Array(slug.utf8)
        let status = slugBytes.withUnsafeBufferPointer { slugBuffer in
            termy_settings_install_theme(slugBuffer.baseAddress, slugBuffer.count)
        }
        try TermyFfiBridge.requireOK("termy_settings_install_theme", status)
    }

    static func prettifyConfig() throws {
        try TermyFfiBridge.requireOK("termy_settings_prettify_config", termy_settings_prettify_config())
    }

    /// Installs the bundled `termy-cli` shim. Returns the success summary, or
    /// throws with the failure reason. `shell == nil` uses $SHELL.
    @discardableResult
    static func installCLI(shell: String? = nil) throws -> String {
        var message = TermyFfiBytes()
        let status: TermyFfiStatus
        if let shell {
            let bytes = Array(shell.utf8)
            status = bytes.withUnsafeBufferPointer { buffer in
                termy_cli_install(buffer.baseAddress, buffer.count, &message)
            }
        } else {
            status = termy_cli_install(nil, 0, &message)
        }
        defer {
            if message.ptr != nil {
                _ = termy_buffer_free(message)
            }
        }
        let text = TermyFfiBridge.string(from: message) ?? ""
        guard status == TERMY_FFI_OK else {
            throw BridgeError.decode(text.isEmpty ? "CLI install failed" : text)
        }
        return text
    }
}
