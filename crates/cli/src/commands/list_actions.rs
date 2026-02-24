const ACTIONS: &[&str] = &[
    "new_tab",
    "close_tab",
    "minimize_window",
    "rename_tab",
    "app_info",
    "native_sdk_example",
    "restart_app",
    "open_config",
    "open_settings",
    "import_colors",
    "switch_theme",
    "zoom_in",
    "zoom_out",
    "zoom_reset",
    "open_search",
    "check_for_updates",
    "quit",
    "toggle_command_palette",
    "copy",
    "paste",
    "close_search",
    "search_next",
    "search_previous",
    "toggle_search_case_sensitive",
    "toggle_search_regex",
    "install_cli",
];

pub fn run() {
    for action in ACTIONS {
        println!("{}", action);
    }
}
