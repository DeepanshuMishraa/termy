const DESKTOP_WEBVIEW_SUPPORTED: bool = cfg!(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux"
));

pub fn browser_tabs_supported() -> bool {
    browser_tabs_supported_for_environment(
        DESKTOP_WEBVIEW_SUPPORTED,
        browser_tabs_blocked_by_linux_wayland(),
    )
}

pub fn browser_tabs_supported_for_environment(
    desktop_webview_supported: bool,
    browser_tabs_blocked_by_environment: bool,
) -> bool {
    desktop_webview_supported && !browser_tabs_blocked_by_environment
}

pub fn browser_tabs_unsupported_message() -> &'static str {
    browser_tabs_unsupported_message_for_environment(
        DESKTOP_WEBVIEW_SUPPORTED,
        browser_tabs_blocked_by_linux_wayland(),
    )
}

pub fn browser_tabs_unsupported_message_for_environment(
    desktop_webview_supported: bool,
    browser_tabs_blocked_by_environment: bool,
) -> &'static str {
    if !desktop_webview_supported {
        return "Browser tabs are not available on this platform yet";
    }
    if browser_tabs_blocked_by_environment {
        return "Browser tabs need an X11 GTK backend on Linux; try launching with GDK_BACKEND=x11 under XWayland";
    }
    "Browser tabs are not available on this platform yet"
}

fn browser_tabs_blocked_by_linux_wayland() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Wry supports child webviews directly on Linux/X11. Wayland needs GTK
        // parenting, but Termy's GPUI window path does not currently expose a
        // GTK container to host the webview.
        linux_browser_tabs_blocked_for_environment(
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("DISPLAY").is_some(),
            std::env::var("GDK_BACKEND").ok().as_deref(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(any(test, target_os = "linux"))]
fn linux_browser_tabs_blocked_for_environment(
    has_wayland_display: bool,
    has_x11_display: bool,
    gdk_backend: Option<&str>,
) -> bool {
    let gtk_backend = gdk_backend.and_then(first_supported_gdk_backend);
    if matches!(gtk_backend, Some("wayland")) {
        return true;
    }
    if matches!(gtk_backend, Some("x11")) {
        return !has_x11_display;
    }
    has_wayland_display || !has_x11_display
}

#[cfg(any(test, target_os = "linux"))]
fn first_supported_gdk_backend(backend: &str) -> Option<&'static str> {
    for entry in backend.split(',').map(str::trim) {
        if entry.eq_ignore_ascii_case("x11") {
            return Some("x11");
        }
        if entry.eq_ignore_ascii_case("wayland") {
            return Some("wayland");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        browser_tabs_supported_for_environment, browser_tabs_unsupported_message,
        browser_tabs_unsupported_message_for_environment,
        linux_browser_tabs_blocked_for_environment,
    };

    #[test]
    fn browser_support_allows_desktop_child_webview_platforms() {
        assert!(browser_tabs_supported_for_environment(true, false));
    }

    #[test]
    fn browser_support_rejects_unknown_webview_platforms() {
        assert!(!browser_tabs_supported_for_environment(false, false));
    }

    #[test]
    fn browser_support_rejects_linux_wayland_without_x11_parenting() {
        assert!(!browser_tabs_supported_for_environment(true, true));
    }

    #[test]
    fn unsupported_message_is_actionable() {
        let message = browser_tabs_unsupported_message();
        assert!(message.contains("Browser tabs"));
        assert!(message.contains("platform") || message.contains("X11"));
    }

    #[test]
    fn unsupported_message_reports_linux_backend_fix() {
        let message = browser_tabs_unsupported_message_for_environment(true, true);
        assert!(message.contains("X11 GTK backend"));
        assert!(message.contains("GDK_BACKEND=x11"));
    }

    #[test]
    fn linux_environment_detection_allows_x11_sessions() {
        assert!(!linux_browser_tabs_blocked_for_environment(
            false, true, None
        ));
        assert!(!linux_browser_tabs_blocked_for_environment(
            true,
            true,
            Some("x11")
        ));
        assert!(!linux_browser_tabs_blocked_for_environment(
            true,
            true,
            Some("x11,wayland")
        ));
        assert!(!linux_browser_tabs_blocked_for_environment(
            true,
            true,
            Some("broadway,x11,wayland")
        ));
    }

    #[test]
    fn linux_environment_detection_rejects_wayland_sessions() {
        assert!(linux_browser_tabs_blocked_for_environment(
            true, false, None
        ));
        assert!(linux_browser_tabs_blocked_for_environment(true, true, None));
        assert!(linux_browser_tabs_blocked_for_environment(
            true,
            true,
            Some("wayland")
        ));
        assert!(linux_browser_tabs_blocked_for_environment(
            true,
            true,
            Some("wayland,x11")
        ));
        assert!(linux_browser_tabs_blocked_for_environment(
            true,
            true,
            Some("broadway,wayland,x11")
        ));
    }

    #[test]
    fn linux_environment_detection_rejects_missing_x11_display() {
        assert!(linux_browser_tabs_blocked_for_environment(
            false, false, None
        ));
        assert!(linux_browser_tabs_blocked_for_environment(
            false,
            false,
            Some("x11")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_real_environment_reports_support_when_expected() {
        if std::env::var_os("TERMY_EXPECT_BROWSER_TABS_SUPPORTED").is_none() {
            return;
        }

        assert!(
            super::browser_tabs_supported(),
            "{}",
            browser_tabs_unsupported_message()
        );
    }
}
