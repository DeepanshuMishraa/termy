use super::*;

const TERMINAL_SCROLL_LINE_MULTIPLIER: f32 = 3.0;
const TERMINAL_SCROLL_PIXELS_PER_LINE: f32 = 24.0;
const MAX_TERMINAL_SCROLL_LINES_PER_EVENT: i32 = 15;

impl TerminalView {
    pub(super) fn has_selection(&self) -> bool {
        matches!((self.selection_anchor, self.selection_head), (Some(anchor), Some(head)) if self.selection_moved || anchor != head)
    }

    pub(super) fn selection_range(&self) -> Option<(CellPos, CellPos)> {
        if !self.has_selection() {
            return None;
        }

        let (anchor, head) = (self.selection_anchor?, self.selection_head?);
        if (head.row, head.col) < (anchor.row, anchor.col) {
            Some((head, anchor))
        } else {
            Some((anchor, head))
        }
    }

    pub(super) fn cell_is_selected(&self, col: usize, row: usize) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };

        let here = (row, col);
        here >= (start.row, start.col) && here <= (end.row, end.col)
    }

    fn write_copy_fallback_input(&mut self, _cx: &mut Context<Self>) {
        #[cfg(not(target_os = "macos"))]
        {
            self.active_terminal().write(&[0x03]);
            self.clear_selection();
            _cx.notify();
        }
    }

    fn write_paste_fallback_input(&mut self, _cx: &mut Context<Self>) {
        #[cfg(not(target_os = "macos"))]
        {
            self.active_terminal().write(&[0x16]);
            self.clear_selection();
            _cx.notify();
        }
    }

    pub(super) fn position_to_cell(
        &self,
        position: gpui::Point<Pixels>,
        clamp: bool,
    ) -> Option<CellPos> {
        let size = self.active_terminal().size();
        if size.cols == 0 || size.rows == 0 {
            return None;
        }

        let mut x: f32 = position.x.into();
        let mut y: f32 = position.y.into();
        x -= self.padding_x;
        y -= self.chrome_height() + self.padding_y;

        let cell_width: f32 = size.cell_width.into();
        let cell_height: f32 = size.cell_height.into();
        if cell_width <= 0.0 || cell_height <= 0.0 {
            return None;
        }

        let mut col = (x / cell_width).floor() as i32;
        let mut row = (y / cell_height).floor() as i32;

        let max_col = i32::from(size.cols) - 1;
        let max_row = i32::from(size.rows) - 1;
        if max_col < 0 || max_row < 0 {
            return None;
        }

        if clamp {
            col = col.clamp(0, max_col);
            row = row.clamp(0, max_row);
        } else if col < 0 || col > max_col || row < 0 || row > max_row {
            return None;
        }

        Some(CellPos {
            col: col as usize,
            row: row as usize,
        })
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let size = self.active_terminal().size();
        let cols = size.cols as usize;
        let rows = size.rows as usize;
        if cols == 0 || rows == 0 {
            return None;
        }

        let mut grid = vec![vec![' '; cols]; rows];
        self.active_terminal().with_term(|term| {
            let content = term.renderable_content();
            for cell in content.display_iter {
                let row = cell.point.line.0;
                if row < 0 {
                    continue;
                }

                let row = row as usize;
                let col = cell.point.column.0;
                if row >= rows || col >= cols {
                    continue;
                }

                let c = cell.cell.c;
                if c != '\0' {
                    grid[row][col] = if c.is_control() { ' ' } else { c };
                }
            }
        });

        let mut lines = Vec::new();
        for row in start.row..=end.row {
            let col_start = if row == start.row { start.col } else { 0 };
            let col_end = if row == end.row {
                end.col
            } else {
                cols.saturating_sub(1)
            };
            let mut line: String = grid[row][col_start..=col_end].iter().collect();
            while line.ends_with(' ') {
                line.pop();
            }
            lines.push(line);
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    pub(super) fn row_text(&self, row: usize) -> Option<Vec<char>> {
        let size = self.active_terminal().size();
        let cols = size.cols as usize;
        let rows = size.rows as usize;
        if cols == 0 || row >= rows {
            return None;
        }

        let mut line = vec![' '; cols];
        self.active_terminal().with_term(|term| {
            let content = term.renderable_content();
            for cell in content.display_iter {
                let cell_row = cell.point.line.0;
                if cell_row < 0 || cell_row as usize != row {
                    continue;
                }

                let col = cell.point.column.0;
                if col >= cols {
                    continue;
                }

                if cell.cell.flags.intersects(
                    Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER | Flags::HIDDEN,
                ) {
                    continue;
                }

                let c = cell.cell.c;
                if c != '\0' {
                    line[col] = if c.is_control() { ' ' } else { c };
                }
            }
        });

        Some(line)
    }

    pub(super) fn link_at_cell(&self, cell: CellPos) -> Option<HoveredLink> {
        let line = self.row_text(cell.row)?;
        let detected = find_link_in_line(&line, cell.col)?;

        Some(HoveredLink {
            row: cell.row,
            start_col: detected.start_col,
            end_col: detected.end_col,
            target: detected.target,
        })
    }

    pub(super) fn open_link(url: &str) -> bool {
        #[cfg(target_os = "macos")]
        {
            return Command::new("open")
                .arg(url)
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
        }
        #[cfg(target_os = "linux")]
        {
            return Command::new("xdg-open")
                .arg(url)
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
        }
        #[cfg(target_os = "windows")]
        {
            return Command::new("cmd")
                .args(["/C", "start", "", url])
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
        }
    }

    pub(super) fn restart_application(&self) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {}", e))?;

        #[cfg(target_os = "macos")]
        {
            let app_bundle = exe
                .ancestors()
                .find(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("app"))
                        .unwrap_or(false)
                })
                .map(PathBuf::from);

            if let Some(app_bundle) = app_bundle {
                let status = Command::new("open")
                    .arg("-n")
                    .arg(&app_bundle)
                    .status()
                    .map_err(|e| format!("failed to launch app bundle: {}", e))?;
                if status.success() {
                    return Ok(());
                }
                return Err(format!("open returned non-success status: {}", status));
            }
        }

        Command::new(&exe)
            .spawn()
            .map_err(|e| format!("failed to spawn executable: {}", e))?;
        Ok(())
    }

    pub(super) fn is_link_modifier(modifiers: gpui::Modifiers) -> bool {
        modifiers.secondary() && !modifiers.alt && !modifiers.function
    }

    pub(super) fn update_zoom(&mut self, next_size: f32, cx: &mut Context<Self>) {
        let clamped = next_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        let current: f32 = self.font_size.into();
        if (current - clamped).abs() < f32::EPSILON {
            return;
        }

        self.font_size = px(clamped);
        // Force cell size recalc so terminal grid reflows at the new zoom.
        self.cell_size = None;
        cx.notify();
    }

    pub(super) fn calculate_cell_size(&mut self, window: &mut Window, _cx: &App) -> Size<Pixels> {
        if let Some(cell_size) = self.cell_size {
            return cell_size;
        }

        let font = Font {
            family: self.font_family.clone(),
            weight: FontWeight::NORMAL,
            ..Default::default()
        };

        // Measure 'M' character width for monospace
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system
            .advance(font_id, self.font_size, 'M')
            .map(|advance| advance.width)
            .unwrap_or(px(9.0));

        let cell_height = self.font_size * self.line_height;

        let cell_size = Size {
            width: cell_width,
            height: cell_height,
        };
        self.cell_size = Some(cell_size);
        cell_size
    }

    pub(super) fn sync_terminal_size(&mut self, window: &Window, cell_size: Size<Pixels>) {
        let viewport = window.viewport_size();
        let viewport_width: f32 = viewport.width.into();
        let viewport_height: f32 = viewport.height.into();
        let cell_width: f32 = cell_size.width.into();
        let cell_height: f32 = cell_size.height.into();

        if cell_width <= 0.0 || cell_height <= 0.0 {
            return;
        }

        let terminal_width = (viewport_width - (self.padding_x * 2.0)).max(cell_width * 2.0);
        let terminal_height =
            (viewport_height - self.chrome_height() - (self.padding_y * 2.0)).max(cell_height);
        let cols = (terminal_width / cell_width).floor().max(2.0) as u16;
        let rows = (terminal_height / cell_height).floor().max(1.0) as u16;

        for tab in &mut self.tabs {
            let current = tab.terminal.size();
            if current.cols != cols
                || current.rows != rows
                || current.cell_width != cell_size.width
                || current.cell_height != cell_size.height
            {
                tab.terminal.resize(TerminalSize {
                    cols,
                    rows,
                    cell_width: cell_size.width,
                    cell_height: cell_size.height,
                });
            }
        }
    }

    pub(super) fn terminal_scroll_delta_to_lines(delta: ScrollDelta) -> i32 {
        let lines = match delta {
            ScrollDelta::Pixels(delta) => {
                let y: f32 = delta.y.into();
                y / TERMINAL_SCROLL_PIXELS_PER_LINE
            }
            ScrollDelta::Lines(delta) => delta.y * TERMINAL_SCROLL_LINE_MULTIPLIER,
        };

        if lines.abs() < f32::EPSILON {
            return 0;
        }

        let magnitude = (lines.abs().ceil() as i32).clamp(1, MAX_TERMINAL_SCROLL_LINES_PER_EVENT);
        if lines.is_sign_negative() {
            -magnitude
        } else {
            magnitude
        }
    }

    fn command_shortcuts_suspended(&self) -> bool {
        self.command_palette_open || self.renaming_tab.is_some()
    }

    pub(super) fn execute_command_action(
        &mut self,
        action: CommandAction,
        respect_shortcut_suspend: bool,
        cx: &mut Context<Self>,
    ) {
        let shortcuts_suspended = respect_shortcut_suspend && self.command_shortcuts_suspended();

        match action {
            CommandAction::ToggleCommandPalette => {
                if self.command_palette_open {
                    self.close_command_palette(cx);
                } else {
                    self.open_command_palette(cx);
                }
            }
            _ if shortcuts_suspended => {}
            CommandAction::Quit => cx.quit(),
            CommandAction::OpenConfig => config::open_config_file(),
            CommandAction::AppInfo => {
                let config_path = self
                    .config_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "unknown".to_string());
                let message = format!(
                    "Termy v{} | {}-{} | config: {}",
                    crate::APP_VERSION,
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    config_path
                );
                termy_toast::info(message);
                cx.notify();
            }
            CommandAction::RestartApp => match self.restart_application() {
                Ok(()) => cx.quit(),
                Err(error) => {
                    termy_toast::error(format!("Restart failed: {}", error));
                    cx.notify();
                }
            },
            CommandAction::RenameTab => {
                if !self.use_tabs {
                    return;
                }

                self.renaming_tab = Some(self.active_tab);
                self.rename_buffer = self.tabs[self.active_tab].title.clone();
                termy_toast::info("Rename mode enabled");
                cx.notify();
            }
            CommandAction::CheckForUpdates => {
                #[cfg(target_os = "macos")]
                {
                    if let Some(updater) = self.auto_updater.as_ref() {
                        AutoUpdater::check(updater.downgrade(), cx);
                    }
                    termy_toast::info("Checking for updates");
                    cx.notify();
                }

                #[cfg(not(target_os = "macos"))]
                {
                    termy_toast::info("Auto updates are only available on macOS");
                    cx.notify();
                }
            }
            CommandAction::NewTab => self.add_tab(cx),
            CommandAction::CloseTab => self.close_active_tab(cx),
            CommandAction::Copy => {
                if let Some(selected) = self.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected));
                } else {
                    self.write_copy_fallback_input(cx);
                }
            }
            CommandAction::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.active_terminal().write(text.as_bytes());
                    self.clear_selection();
                    cx.notify();
                } else {
                    self.write_paste_fallback_input(cx);
                }
            }
            CommandAction::ZoomIn => {
                let current: f32 = self.font_size.into();
                self.update_zoom(current + ZOOM_STEP, cx);
            }
            CommandAction::ZoomOut => {
                let current: f32 = self.font_size.into();
                self.update_zoom(current - ZOOM_STEP, cx);
            }
            CommandAction::ZoomReset => self.update_zoom(self.base_font_size, cx),
        }
    }

    pub(super) fn handle_toggle_command_palette_action(
        &mut self,
        _: &commands::ToggleCommandPalette,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::ToggleCommandPalette, true, cx);
    }

    pub(super) fn handle_app_info_action(
        &mut self,
        _: &commands::AppInfo,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::AppInfo, true, cx);
    }

    pub(super) fn handle_restart_app_action(
        &mut self,
        _: &commands::RestartApp,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::RestartApp, true, cx);
    }

    pub(super) fn handle_rename_tab_action(
        &mut self,
        _: &commands::RenameTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::RenameTab, true, cx);
    }

    pub(super) fn handle_check_for_updates_action(
        &mut self,
        _: &commands::CheckForUpdates,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::CheckForUpdates, true, cx);
    }

    pub(super) fn handle_new_tab_action(
        &mut self,
        _: &commands::NewTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::NewTab, true, cx);
    }

    pub(super) fn handle_close_tab_action(
        &mut self,
        _: &commands::CloseTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::CloseTab, true, cx);
    }

    pub(super) fn handle_copy_action(
        &mut self,
        _: &commands::Copy,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::Copy, true, cx);
    }

    pub(super) fn handle_paste_action(
        &mut self,
        _: &commands::Paste,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::Paste, true, cx);
    }

    pub(super) fn handle_zoom_in_action(
        &mut self,
        _: &commands::ZoomIn,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::ZoomIn, true, cx);
    }

    pub(super) fn handle_zoom_out_action(
        &mut self,
        _: &commands::ZoomOut,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::ZoomOut, true, cx);
    }

    pub(super) fn handle_zoom_reset_action(
        &mut self,
        _: &commands::ZoomReset,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_command_action(CommandAction::ZoomReset, true, cx);
    }

    pub(super) fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let key_char = event.keystroke.key_char.as_deref();

        if self.command_palette_open {
            self.handle_command_palette_key_down(key, key_char, modifiers, cx);
            return;
        }

        if self.renaming_tab.is_some() {
            match key {
                "enter" => {
                    self.commit_rename_tab(cx);
                    return;
                }
                "escape" => {
                    self.cancel_rename_tab(cx);
                    return;
                }
                "backspace" => {
                    self.rename_buffer.pop();
                    cx.notify();
                    return;
                }
                "space"
                    if !modifiers.control
                        && !modifiers.alt
                        && !modifiers.function
                        && !modifiers.platform =>
                {
                    if self.rename_buffer.chars().count() < MAX_TAB_TITLE_CHARS {
                        self.rename_buffer.push(' ');
                        cx.notify();
                    }
                    return;
                }
                _ if key.len() == 1
                    && key_char.is_some()
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.function
                    && !modifiers.platform =>
                {
                    if let Some(input) = key_char {
                        let input_len = input.chars().count();
                        if input_len > 0
                            && self.rename_buffer.chars().count() + input_len <= MAX_TAB_TITLE_CHARS
                        {
                            self.rename_buffer.push_str(input);
                            cx.notify();
                        }
                    }
                    return;
                }
                _ => return,
            }
        }

        if let Some(input) = keystroke_to_input(&event.keystroke) {
            self.active_terminal().write(&input);
            self.clear_selection();
            // Request a redraw to show the typed character
            cx.notify();
        }
    }

    pub(super) fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Focus the terminal on click
        self.focus_handle.focus(window, cx);

        if event.button != MouseButton::Left {
            return;
        }

        if Self::is_link_modifier(event.modifiers) {
            if let Some(cell) = self.position_to_cell(event.position, false) {
                if let Some(link) = self.link_at_cell(cell) {
                    if !Self::open_link(&link.target) {
                        termy_toast::error("Failed to open link");
                    }
                    if self.clear_hovered_link() {
                        cx.notify();
                    }
                    return;
                }
            }
        }

        let Some(cell) = self.position_to_cell(event.position, false) else {
            self.clear_selection();
            self.clear_hovered_link();
            cx.notify();
            return;
        };

        self.selection_anchor = Some(cell);
        self.selection_head = Some(cell);
        self.selection_dragging = true;
        self.selection_moved = false;
        self.clear_hovered_link();
        cx.notify();
    }

    pub(super) fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selection_dragging || !event.dragging() {
            if Self::is_link_modifier(event.modifiers) {
                let next = self
                    .position_to_cell(event.position, false)
                    .and_then(|cell| self.link_at_cell(cell));
                if self.hovered_link != next {
                    self.hovered_link = next;
                    cx.notify();
                }
            } else if self.clear_hovered_link() {
                cx.notify();
            }
            return;
        }

        let Some(next_cell) = self.position_to_cell(event.position, true) else {
            return;
        };

        if self.selection_head != Some(next_cell) {
            self.selection_head = Some(next_cell);
            if self.selection_anchor != self.selection_head {
                self.selection_moved = true;
            }
            self.clear_hovered_link();
            cx.notify();
        }
    }

    pub(super) fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !self.selection_dragging {
            return;
        }

        if let Some(next_cell) = self.position_to_cell(event.position, true) {
            self.selection_head = Some(next_cell);
            if self.selection_anchor != self.selection_head {
                self.selection_moved = true;
            }
        }

        self.selection_dragging = false;
        if !self.selection_moved {
            self.clear_selection();
        }
        self.clear_hovered_link();
        cx.notify();
    }

    pub(super) fn handle_titlebar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }

        if event.click_count == 2 {
            #[cfg(target_os = "macos")]
            window.titlebar_double_click();
            #[cfg(not(target_os = "macos"))]
            window.zoom_window();
            return;
        }

        window.start_window_move();
    }

    pub(super) fn handle_terminal_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta_lines = Self::terminal_scroll_delta_to_lines(event.delta);
        if delta_lines == 0 {
            return;
        }

        if self.active_terminal().scroll_display(delta_lines) {
            cx.notify();
        }
    }

    pub(super) fn tab_bar_height(&self) -> f32 {
        if self.show_tab_bar() {
            TABBAR_HEIGHT
        } else {
            0.0
        }
    }

    pub(super) fn titlebar_height(&self) -> f32 {
        #[cfg(target_os = "windows")]
        {
            0.0
        }
        #[cfg(not(target_os = "windows"))]
        {
            TITLEBAR_HEIGHT
        }
    }

    pub(super) fn update_banner_height(&self) -> f32 {
        #[cfg(target_os = "macos")]
        if self.show_update_banner {
            return UPDATE_BANNER_HEIGHT;
        }
        0.0
    }

    pub(super) fn chrome_height(&self) -> f32 {
        self.titlebar_height() + self.tab_bar_height() + self.update_banner_height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_scroll_lines_scale_line_delta() {
        let delta = ScrollDelta::Lines(gpui::point(0.0, 1.0));
        assert_eq!(TerminalView::terminal_scroll_delta_to_lines(delta), 3);
    }

    #[test]
    fn terminal_scroll_lines_scale_pixel_delta() {
        let delta = ScrollDelta::Pixels(gpui::point(px(0.0), px(48.0)));
        assert_eq!(TerminalView::terminal_scroll_delta_to_lines(delta), 2);
    }

    #[test]
    fn terminal_scroll_lines_preserve_sign() {
        let delta = ScrollDelta::Lines(gpui::point(0.0, -1.0));
        assert_eq!(TerminalView::terminal_scroll_delta_to_lines(delta), -3);
    }

    #[test]
    fn terminal_scroll_lines_clamp_large_deltas() {
        let delta = ScrollDelta::Lines(gpui::point(0.0, 999.0));
        assert_eq!(
            TerminalView::terminal_scroll_delta_to_lines(delta),
            MAX_TERMINAL_SCROLL_LINES_PER_EVENT
        );
    }

    #[test]
    fn terminal_scroll_lines_ignore_zero_delta() {
        let delta = ScrollDelta::Pixels(gpui::point(px(0.0), px(0.0)));
        assert_eq!(TerminalView::terminal_scroll_delta_to_lines(delta), 0);
    }
}
