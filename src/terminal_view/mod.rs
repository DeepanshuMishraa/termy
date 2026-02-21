use crate::colors::TerminalColors;
use crate::commands::{self, CommandAction};
use crate::config::{self, AppConfig, TabTitleConfig, TabTitleSource};
use crate::keybindings;
use alacritty_terminal::term::cell::Flags;
use flume::{Sender, bounded};
use gpui::{
    AnyElement, App, AsyncApp, ClipboardItem, Context, Element, FocusHandle, Focusable, Font,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, ScrollWheelEvent, SharedString,
    Size, StatefulInteractiveElement, Styled, TouchPhase, UniformListScrollHandle, WeakEntity,
    Window, WindowControlArea, div, px,
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, SystemTime},
};
use termy_terminal_ui::{
    CellRenderInfo, TabTitleShellIntegration, Terminal, TerminalEvent, TerminalGrid,
    TerminalRuntimeConfig, TerminalSize, WorkingDirFallback as RuntimeWorkingDirFallback,
    find_link_in_line, keystroke_to_input,
};
use termy_toast::ToastManager;

#[cfg(target_os = "macos")]
use gpui::{AppContext, Entity};
#[cfg(target_os = "macos")]
use termy_auto_update::{AutoUpdater, UpdateState};

mod command_palette;
mod inline_input;
mod interaction;
mod render;
mod tabs;
mod titles;
#[cfg(target_os = "macos")]
mod update_toasts;

use inline_input::{InlineInputElement, InlineInputState, InlineInputTarget};

const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 40.0;
const ZOOM_STEP: f32 = 1.0;
const TITLEBAR_HEIGHT: f32 = 34.0;
const TABBAR_HEIGHT: f32 = 40.0;
const TITLEBAR_PLUS_SIZE: f32 = 22.0;
const WINDOWS_TITLEBAR_BUTTON_WIDTH: f32 = 46.0;
const WINDOWS_TITLEBAR_CONTROLS_WIDTH: f32 = WINDOWS_TITLEBAR_BUTTON_WIDTH * 3.0;
const TITLEBAR_SIDE_PADDING: f32 = 12.0;
const TAB_HORIZONTAL_PADDING: f32 = 12.0;
const TAB_PILL_HEIGHT: f32 = 32.0;
const TAB_PILL_NORMAL_PADDING: f32 = 10.0;
const TAB_PILL_COMPACT_PADDING: f32 = 6.0;
const TAB_PILL_COMPACT_THRESHOLD: f32 = 120.0;
const TAB_PILL_GAP: f32 = 8.0;
const TAB_CLOSE_HITBOX: f32 = 22.0;
const TAB_INACTIVE_CLOSE_MIN_WIDTH: f32 = 120.0;
const MAX_TAB_TITLE_CHARS: usize = 96;
const DEFAULT_TAB_TITLE: &str = "Terminal";
const COMMAND_TITLE_DELAY_MS: u64 = 250;
const CONFIG_WATCH_INTERVAL_MS: u64 = 750;
const SELECTION_BG_ALPHA: f32 = 0.35;
const DIM_TEXT_FACTOR: f32 = 0.66;
#[cfg(target_os = "macos")]
const UPDATE_BANNER_HEIGHT: f32 = 32.0;
const COMMAND_PALETTE_WIDTH: f32 = 640.0;
const COMMAND_PALETTE_MAX_ITEMS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellPos {
    col: usize,
    row: usize,
}

#[derive(Clone, Copy, Debug)]
struct TabBarLayout {
    tab_pill_width: f32,
    tab_padding_x: f32,
}

struct TerminalTab {
    terminal: Terminal,
    manual_title: Option<String>,
    explicit_title: Option<String>,
    shell_title: Option<String>,
    pending_command_title: Option<String>,
    pending_command_token: u64,
    title: String,
}

impl TerminalTab {
    fn new(terminal: Terminal) -> Self {
        Self {
            terminal,
            manual_title: None,
            explicit_title: None,
            shell_title: None,
            pending_command_title: None,
            pending_command_token: 0,
            title: DEFAULT_TAB_TITLE.to_string(),
        }
    }
}

enum ExplicitTitlePayload {
    Prompt(String),
    Command(String),
    Title(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HoveredLink {
    row: usize,
    start_col: usize,
    end_col: usize,
    target: String,
}

#[derive(Clone, Copy, Debug)]
struct CommandPaletteItem {
    title: &'static str,
    keywords: &'static str,
    action: CommandAction,
}

/// The main terminal view component
pub struct TerminalView {
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    renaming_tab: Option<usize>,
    rename_input: InlineInputState,
    event_wakeup_tx: Sender<()>,
    focus_handle: FocusHandle,
    colors: TerminalColors,
    use_tabs: bool,
    tab_title: TabTitleConfig,
    tab_shell_integration: TabTitleShellIntegration,
    configured_working_dir: Option<String>,
    terminal_runtime: TerminalRuntimeConfig,
    config_path: Option<PathBuf>,
    config_last_modified: Option<SystemTime>,
    font_family: SharedString,
    base_font_size: f32,
    font_size: Pixels,
    transparent_background_opacity: f32,
    padding_x: f32,
    padding_y: f32,
    mouse_scroll_multiplier: f32,
    line_height: f32,
    selection_anchor: Option<CellPos>,
    selection_head: Option<CellPos>,
    selection_dragging: bool,
    selection_moved: bool,
    hovered_link: Option<HoveredLink>,
    hovered_toast: Option<u64>,
    toast_manager: ToastManager,
    command_palette_open: bool,
    inline_input_target: Option<InlineInputTarget>,
    command_palette_input: InlineInputState,
    command_palette_selected: usize,
    command_palette_scroll_handle: UniformListScrollHandle,
    command_palette_show_keybinds: bool,
    terminal_scroll_accumulator_y: f32,
    /// Cached cell dimensions
    cell_size: Option<Size<Pixels>>,
    #[cfg(target_os = "macos")]
    auto_updater: Option<Entity<AutoUpdater>>,
    #[cfg(target_os = "macos")]
    show_update_banner: bool,
    #[cfg(target_os = "macos")]
    last_notified_update_state: Option<UpdateState>,
}

impl TerminalView {
    fn runtime_config_from_app_config(config: &AppConfig) -> TerminalRuntimeConfig {
        let working_dir_fallback = match config.working_dir_fallback {
            config::WorkingDirFallback::Home => RuntimeWorkingDirFallback::Home,
            config::WorkingDirFallback::Process => RuntimeWorkingDirFallback::Process,
        };

        TerminalRuntimeConfig {
            shell: config.shell.clone(),
            term: config.term.clone(),
            colorterm: config.colorterm.clone(),
            working_dir_fallback,
        }
    }

    fn config_last_modified(path: &PathBuf) -> Option<SystemTime> {
        fs::metadata(path).ok()?.modified().ok()
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let (event_wakeup_tx, event_wakeup_rx) = bounded(1);

        // Focus the terminal immediately
        focus_handle.focus(window, cx);

        // Process terminal events only when terminals signal activity.
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while event_wakeup_rx.recv_async().await.is_ok() {
                while event_wakeup_rx.try_recv().is_ok() {}
                let result = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        if view.process_terminal_events(cx) {
                            cx.notify();
                        }
                    })
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

        // Poll config file timestamp and hot-reload UI settings on change.
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_millis(CONFIG_WATCH_INTERVAL_MS)).await;
                let result = cx.update(|cx| {
                    this.update(cx, |view, cx| {
                        if view.reload_config_if_changed(cx) {
                            cx.notify();
                        }
                    })
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

        let config = AppConfig::load_or_create();
        let config_path = config::ensure_config_file();
        let config_last_modified = config_path.as_ref().and_then(Self::config_last_modified);
        let colors = TerminalColors::from_theme(&config.theme);
        let base_font_size = config.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let padding_x = config.padding_x.max(0.0);
        let padding_y = config.padding_y.max(0.0);
        let configured_working_dir = config.working_dir.clone();
        let tab_title = config.tab_title.clone();
        let tab_shell_integration = TabTitleShellIntegration {
            enabled: tab_title.shell_integration,
            explicit_prefix: tab_title.explicit_prefix.clone(),
        };
        let terminal_runtime = Self::runtime_config_from_app_config(&config);
        let terminal = Terminal::new(
            TerminalSize::default(),
            configured_working_dir.as_deref(),
            Some(event_wakeup_tx.clone()),
            Some(&tab_shell_integration),
            Some(&terminal_runtime),
        )
        .expect("Failed to create terminal");

        let mut view = Self {
            tabs: vec![TerminalTab::new(terminal)],
            active_tab: 0,
            renaming_tab: None,
            rename_input: InlineInputState::new(String::new()),
            event_wakeup_tx,
            focus_handle,
            colors,
            use_tabs: config.use_tabs,
            tab_title,
            tab_shell_integration,
            configured_working_dir,
            terminal_runtime,
            config_path,
            config_last_modified,
            font_family: config.font_family.into(),
            base_font_size,
            font_size: px(base_font_size),
            transparent_background_opacity: config.transparent_background_opacity,
            padding_x,
            padding_y,
            mouse_scroll_multiplier: config.mouse_scroll_multiplier,
            line_height: 1.4,
            selection_anchor: None,
            selection_head: None,
            selection_dragging: false,
            selection_moved: false,
            hovered_link: None,
            hovered_toast: None,
            toast_manager: ToastManager::new(),
            command_palette_open: false,
            inline_input_target: None,
            command_palette_input: InlineInputState::new(String::new()),
            command_palette_selected: 0,
            command_palette_scroll_handle: UniformListScrollHandle::new(),
            command_palette_show_keybinds: config.command_palette_show_keybinds,
            terminal_scroll_accumulator_y: 0.0,
            cell_size: None,
            #[cfg(target_os = "macos")]
            auto_updater: None,
            #[cfg(target_os = "macos")]
            show_update_banner: false,
            #[cfg(target_os = "macos")]
            last_notified_update_state: None,
        };
        view.refresh_tab_title(0);

        #[cfg(target_os = "macos")]
        {
            let updater = cx.new(|_| AutoUpdater::new(crate::APP_VERSION));
            cx.observe(&updater, |_, _, cx| cx.notify()).detach();
            let weak = updater.downgrade();
            cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                smol::Timer::after(Duration::from_millis(5000)).await;
                let _ = cx.update(|cx| AutoUpdater::check(weak, cx));
            })
            .detach();
            view.auto_updater = Some(updater);
        }

        view
    }

    fn apply_runtime_config(&mut self, config: AppConfig, cx: &mut Context<Self>) -> bool {
        keybindings::install_keybindings(cx, &config);
        self.colors = TerminalColors::from_theme(&config.theme);
        self.use_tabs = config.use_tabs;
        self.tab_title = config.tab_title.clone();
        self.tab_shell_integration = TabTitleShellIntegration {
            enabled: self.tab_title.shell_integration,
            explicit_prefix: self.tab_title.explicit_prefix.clone(),
        };
        self.configured_working_dir = config.working_dir.clone();
        self.terminal_runtime = Self::runtime_config_from_app_config(&config);
        self.font_family = config.font_family.into();
        self.base_font_size = config.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.font_size = px(self.base_font_size);
        self.cell_size = None;
        self.transparent_background_opacity = config.transparent_background_opacity;
        self.padding_x = config.padding_x.max(0.0);
        self.padding_y = config.padding_y.max(0.0);
        self.mouse_scroll_multiplier = config.mouse_scroll_multiplier;
        self.command_palette_show_keybinds = config.command_palette_show_keybinds;

        for index in 0..self.tabs.len() {
            self.refresh_tab_title(index);
        }

        true
    }

    fn reload_config_if_changed(&mut self, cx: &mut Context<Self>) -> bool {
        let path = match self.config_path.clone() {
            Some(path) => path,
            None => {
                self.config_path = config::ensure_config_file();
                match self.config_path.clone() {
                    Some(path) => path,
                    None => return false,
                }
            }
        };

        let Some(modified) = Self::config_last_modified(&path) else {
            return false;
        };

        if let Some(last) = self.config_last_modified
            && modified <= last
        {
            return false;
        }

        self.config_last_modified = Some(modified);
        let config = AppConfig::load_or_create();
        let changed = self.apply_runtime_config(config, cx);
        if changed {
            termy_toast::info("Configuration reloaded");
        }
        changed
    }

    fn process_terminal_events(&mut self, cx: &mut Context<Self>) -> bool {
        let mut should_redraw = false;
        let active_tab = self.active_tab;

        for index in 0..self.tabs.len() {
            let events = self.tabs[index].terminal.process_events();
            for event in events {
                match event {
                    TerminalEvent::Wakeup | TerminalEvent::Bell | TerminalEvent::Exit => {
                        if index == active_tab {
                            should_redraw = true;
                        }
                    }
                    TerminalEvent::Title(title) => {
                        if self.apply_terminal_title(index, &title, cx)
                            && (index == active_tab || self.show_tab_bar())
                        {
                            should_redraw = true;
                        }
                    }
                    TerminalEvent::ResetTitle => {
                        if self.clear_terminal_titles(index)
                            && (index == active_tab || self.show_tab_bar())
                        {
                            should_redraw = true;
                        }
                    }
                }
            }
        }

        should_redraw
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_head = None;
        self.selection_dragging = false;
        self.selection_moved = false;
    }

    fn clear_hovered_link(&mut self) -> bool {
        if self.hovered_link.is_some() {
            self.hovered_link = None;
            true
        } else {
            false
        }
    }

    fn show_tab_bar(&self) -> bool {
        self.use_tabs && self.tabs.len() > 1
    }

    fn active_terminal(&self) -> &Terminal {
        &self.tabs[self.active_tab].terminal
    }
}
