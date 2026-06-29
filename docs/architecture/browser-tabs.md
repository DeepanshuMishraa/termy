# Browser Tabs

Termy browser tabs use Wry native child webviews hosted inside the GPUI window.
The GPUI layer renders tab chrome and the URL bar; the native webview is created
only for visible browser panes and is resized or hidden during render sync.

## Platform Support

- macOS: Wry hosts a WebKit `NSView` child view.
- Windows: Wry hosts a WebView2 child window. The Windows setup package checks
  for the WebView2 Runtime and runs the Evergreen Bootstrapper when needed.
- Linux: Wry hosts a WebKitGTK child window on X11. Wayland sessions are
  blocked until Termy has a GTK container host for `build_gtk`; mixed
  Wayland/XWayland sessions need `GDK_BACKEND=x11` before Termy advertises
  embedded browser support. Linux package launchers set that automatically when
  `DISPLAY` exists and `GDK_BACKEND` is unset.

Linux builds need WebKitGTK/GTK installed at runtime. The desktop app
initializes GTK before creating the first Linux browser webview, verifies GTK is
using an X11 backend, and pumps pending GTK events during browser webview sync.

If native webview creation fails at runtime, the browser pane shows the error and
offers two actions:

- Open the current URL in the system browser.
- Retry native webview creation.

Command availability, menus, settings, and command-palette messaging share the
same support detection through `termy_command_core`.
