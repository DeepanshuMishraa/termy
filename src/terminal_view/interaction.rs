use super::*;

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

    pub(super) fn is_copy_shortcut(key: &str, modifiers: gpui::Modifiers) -> bool {
        #[cfg(target_os = "macos")]
        {
            modifiers.platform
                && !modifiers.control
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("c")
        }
        #[cfg(not(target_os = "macos"))]
        {
            modifiers.control
                && modifiers.shift
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("c")
        }
    }

    pub(super) fn is_paste_shortcut(key: &str, modifiers: gpui::Modifiers) -> bool {
        #[cfg(target_os = "macos")]
        {
            modifiers.platform
                && !modifiers.control
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("v")
        }
        #[cfg(not(target_os = "macos"))]
        {
            modifiers.control
                && modifiers.shift
                && !modifiers.alt
                && !modifiers.function
                && key.eq_ignore_ascii_case("v")
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

    pub(super) fn calculate_cell_size(&self, window: &mut Window, _cx: &App) -> Size<Pixels> {
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

        Size {
            width: cell_width,
            height: cell_height,
        }
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

        let current = self.active_terminal().size();
        if current.cols != cols || current.rows != rows {
            self.active_terminal_mut().resize(TerminalSize {
                cols,
                rows,
                cell_width: cell_size.width,
                cell_height: cell_size.height,
            });
        }
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

        if Self::is_command_palette_shortcut(key, modifiers) {
            if self.command_palette_open {
                self.close_command_palette(cx);
            } else {
                self.open_command_palette(cx);
            }
            return;
        }

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

        if Self::is_copy_shortcut(key, modifiers) {
            if let Some(selected) = self.selected_text() {
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
            }
            return;
        }

        if Self::is_paste_shortcut(key, modifiers) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.active_terminal().write(text.as_bytes());
                self.clear_selection();
                cx.notify();
            }
            return;
        }

        if self.use_tabs
            && modifiers.secondary()
            && !modifiers.alt
            && !modifiers.function
            && key.eq_ignore_ascii_case("w")
        {
            self.close_active_tab(cx);
            return;
        }

        if self.use_tabs
            && modifiers.secondary()
            && !modifiers.alt
            && !modifiers.function
            && key.eq_ignore_ascii_case("t")
        {
            self.add_tab(cx);
            return;
        }

        if modifiers.secondary() && !modifiers.alt && !modifiers.function {
            let current: f32 = self.font_size.into();
            match key {
                "=" | "+" | "plus" => {
                    self.update_zoom(current + ZOOM_STEP, cx);
                    return;
                }
                "-" | "_" | "minus" => {
                    self.update_zoom(current - ZOOM_STEP, cx);
                    return;
                }
                "0" => {
                    self.update_zoom(self.base_font_size, cx);
                    return;
                }
                _ => {}
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

    pub(super) fn handle_tabbar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !self.show_tab_bar() {
            return;
        }

        let mut x: f32 = event.position.x.into();
        x -= TAB_HORIZONTAL_PADDING;
        if x < 0.0 {
            return;
        }

        let viewport = window.viewport_size();
        let layout = self.tab_bar_layout(viewport.width.into());
        let index = (x / layout.slot_width).floor() as usize;
        if index >= self.tabs.len() {
            return;
        }

        let x_in_slot = x - (index as f32 * layout.slot_width);
        if x_in_slot > layout.tab_pill_width {
            return;
        }

        let is_active = index == self.active_tab;
        let show_close =
            Self::tab_shows_close(layout.tab_pill_width, is_active, layout.tab_padding_x);
        if show_close {
            let close_left =
                (layout.tab_pill_width - layout.tab_padding_x - TAB_CLOSE_HITBOX).max(0.0);
            let close_right = (layout.tab_pill_width - layout.tab_padding_x).max(0.0);
            if x_in_slot >= close_left && x_in_slot <= close_right {
                self.close_tab(index, cx);
                return;
            }
        }

        self.switch_tab(index, cx);
    }

    pub(super) fn handle_titlebar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }

        if self.use_tabs && !cfg!(target_os = "windows") {
            let viewport = window.viewport_size();
            let viewport_width: f32 = viewport.width.into();
            let x: f32 = event.position.x.into();
            let y: f32 = event.position.y.into();
            let plus_left = viewport_width - TITLEBAR_SIDE_PADDING - TITLEBAR_PLUS_SIZE;
            let plus_top = (self.titlebar_height() - TITLEBAR_PLUS_SIZE) * 0.5;
            let plus_right = plus_left + TITLEBAR_PLUS_SIZE;
            let plus_bottom = plus_top + TITLEBAR_PLUS_SIZE;

            if x >= plus_left && x <= plus_right && y >= plus_top && y <= plus_bottom {
                self.add_tab(cx);
                return;
            }
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
