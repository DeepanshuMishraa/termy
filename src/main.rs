#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod colors;
mod config;
mod terminal;
mod terminal_view;
mod themes;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, actions,
    prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::SystemMenuType;
use terminal_view::TerminalView;

actions!(terminal, [Quit, OpenConfig]);

const MIN_WINDOW_WIDTH: f32 = 480.0;
const MIN_WINDOW_HEIGHT: f32 = 320.0;

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
        let window_width = app_config.window_width.max(MIN_WINDOW_WIDTH);
        let window_height = app_config.window_height.max(MIN_WINDOW_HEIGHT);
        let bounds = Bounds::centered(None, size(px(window_width), px(window_height)), cx);

        #[cfg(target_os = "macos")]
        let titlebar = gpui::TitlebarOptions {
            title: Some("Termy".into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui::point(px(12.0), px(10.0))),
            ..Default::default()
        };
        #[cfg(not(target_os = "macos"))]
        let titlebar = gpui::TitlebarOptions {
            title: Some("Termy".into()),
            appears_transparent: true,
            ..Default::default()
        };

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(titlebar),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TerminalView::new(window, cx)),
        )
        .unwrap();
    });
}
