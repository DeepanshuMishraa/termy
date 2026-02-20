#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod colors;
mod config;
mod terminal_view;

#[cfg(target_os = "macos")]
use gpui::SystemMenuType;
use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, actions,
    prelude::*, px, size,
};
use terminal_view::TerminalView;

actions!(terminal, [Quit, OpenConfig]);

pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIN_WINDOW_WIDTH: f32 = 480.0;
const MIN_WINDOW_HEIGHT: f32 = 320.0;
#[cfg(target_os = "windows")]
const LEGACY_DEFAULT_WINDOW_WIDTH: f32 = 1100.0;
#[cfg(target_os = "windows")]
const LEGACY_DEFAULT_WINDOW_HEIGHT: f32 = 720.0;
#[cfg(target_os = "windows")]
const WINDOWS_DEFAULT_WINDOW_WIDTH: f32 = 1280.0;
#[cfg(target_os = "windows")]
const WINDOWS_DEFAULT_WINDOW_HEIGHT: f32 = 820.0;

#[cfg(target_os = "macos")]
const QUIT_KEYBINDING: &str = "cmd-q";
#[cfg(not(target_os = "macos"))]
const QUIT_KEYBINDING: &str = "ctrl-q";

#[cfg(target_os = "macos")]
const OPEN_CONFIG_KEYBINDING: &str = "cmd-,";
#[cfg(not(target_os = "macos"))]
const OPEN_CONFIG_KEYBINDING: &str = "ctrl-,";

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        cx.bind_keys([KeyBinding::new(QUIT_KEYBINDING, Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new(OPEN_CONFIG_KEYBINDING, OpenConfig, None)]);
        cx.on_action(|_: &OpenConfig, _cx| config::open_config_file());

        #[cfg(target_os = "macos")]
        let menu_items = vec![
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Preferences...", OpenConfig),
            MenuItem::action("Quit", Quit),
        ];
        #[cfg(not(target_os = "macos"))]
        let menu_items = vec![
            MenuItem::separator(),
            MenuItem::action("Preferences...", OpenConfig),
            MenuItem::action("Quit", Quit),
        ];

        cx.set_menus(vec![Menu {
            name: "Termy".into(),
            items: menu_items,
        }]);

        let app_config = config::AppConfig::load_or_create();
        let window_width = app_config.window_width;
        let window_height = app_config.window_height;
        #[cfg(target_os = "windows")]
        let (window_width, window_height) =
            if (window_width - LEGACY_DEFAULT_WINDOW_WIDTH).abs() < f32::EPSILON
                && (window_height - LEGACY_DEFAULT_WINDOW_HEIGHT).abs() < f32::EPSILON
            {
                (WINDOWS_DEFAULT_WINDOW_WIDTH, WINDOWS_DEFAULT_WINDOW_HEIGHT)
            } else {
                (window_width, window_height)
            };
        let window_width = window_width.max(MIN_WINDOW_WIDTH);
        let window_height = window_height.max(MIN_WINDOW_HEIGHT);
        let bounds = Bounds::centered(None, size(px(window_width), px(window_height)), cx);

        #[cfg(target_os = "macos")]
        let titlebar = Some(gpui::TitlebarOptions {
            title: Some("Termy".into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui::point(px(12.0), px(10.0))),
            ..Default::default()
        });
        #[cfg(target_os = "windows")]
        let titlebar = Some(gpui::TitlebarOptions {
            title: Some("Termy".into()),
            ..Default::default()
        });
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let titlebar = Some(gpui::TitlebarOptions {
            title: Some("Termy".into()),
            appears_transparent: true,
            ..Default::default()
        });

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TerminalView::new(window, cx)),
        )
        .unwrap();
    });
}
