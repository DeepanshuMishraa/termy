mod colors;
mod config;
mod terminal;
mod themes;
mod terminal_view;

use gpui::{App, Application, Bounds, KeyBinding, Menu, MenuItem, SystemMenuType, WindowBounds, WindowOptions, actions, prelude::*, px, size};
use terminal_view::TerminalView;

actions!(terminal, [Quit, OpenConfig]);

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-,", OpenConfig, None)]);
        cx.on_action(|_: &OpenConfig, _cx| config::open_config_file());
        cx.set_menus(vec![Menu {
            name: "gpui-terminal".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Preferences...", OpenConfig),
                MenuItem::action("Quit", Quit),
            ],
        }]);

        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| TerminalView::new(window, cx)),
        )
        .unwrap();
    });
}
