# Browser Tabs

Termy browser tabs use Wry native child webviews hosted inside the GPUI window.
The GPUI layer renders tab chrome and the URL bar; the native webview is created
only for visible browser panes and is resized or hidden during render sync.

## Platform Support

- macOS: Wry hosts a WebKit `NSView` child view.
- Windows and Linux: browser tabs are disabled. The desktop app reports the
  capability as unsupported, hides browser-tab commands, and does not compile
  Wry into either target.

On macOS, if native webview creation fails at runtime, the browser pane shows
the error and offers two actions:

- Open the current URL in the system browser.
- Retry native webview creation.

Command availability, menus, settings, and command-palette messaging share the
same support detection through `termy_command_core`.
