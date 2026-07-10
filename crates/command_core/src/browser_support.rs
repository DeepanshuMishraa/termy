const BROWSER_TABS_SUPPORTED: bool = cfg!(target_os = "macos");

pub const fn browser_tabs_supported() -> bool {
    BROWSER_TABS_SUPPORTED
}

pub const fn browser_tabs_unsupported_message() -> &'static str {
    "Browser tabs are only available on macOS"
}

#[cfg(test)]
mod tests {
    use super::{browser_tabs_supported, browser_tabs_unsupported_message};

    #[test]
    fn browser_support_is_macos_only() {
        assert_eq!(browser_tabs_supported(), cfg!(target_os = "macos"));
    }

    #[test]
    fn unsupported_message_names_the_supported_platform() {
        assert_eq!(
            browser_tabs_unsupported_message(),
            "Browser tabs are only available on macOS"
        );
    }
}
