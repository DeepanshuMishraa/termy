use super::*;
use gpui::{
    Bounds, ElementInputHandler, Entity, EntityInputHandler, IntoElement, Pixels, ScrollStrategy,
    UTF16Selection, Window, canvas,
};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InlineInputTarget {
    CommandPalette,
    TabRename,
}

#[derive(Clone, Debug)]
pub(super) struct InlineInputState {
    text: String,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
}

impl InlineInputState {
    pub(super) fn new(text: String) -> Self {
        let mut state = Self {
            text,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
        };
        state.move_to_end();
        state
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn set_text(&mut self, text: String) {
        self.text = text;
        self.marked_range = None;
        self.selection_reversed = false;
        self.move_to_end();
    }

    pub(super) fn clear(&mut self) {
        self.set_text(String::new());
    }

    pub(super) fn move_to_end(&mut self) {
        let end = self.text.len();
        self.selected_range = end..end;
    }

    pub(super) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub(super) fn selected_range(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    pub(super) fn select_all(&mut self) {
        self.selection_reversed = false;
        self.selected_range = 0..self.text.len();
    }

    pub(super) fn text_with_cursor(&self) -> String {
        if !self.selected_range.is_empty() {
            return self.text.clone();
        }

        let cursor = self.cursor_offset().min(self.text.len());
        let mut rendered = String::with_capacity(self.text.len() + 1);
        rendered.push_str(&self.text[..cursor]);
        rendered.push('▌');
        rendered.push_str(&self.text[cursor..]);
        rendered
    }

    fn clamp_utf8_index(text: &str, index: usize) -> usize {
        let mut index = index.min(text.len());
        while index > 0 && !text.is_char_boundary(index) {
            index -= 1;
        }
        index
    }

    fn utf16_to_utf8_in_text(text: &str, utf16_offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in text.chars() {
            if utf16_count >= utf16_offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        Self::clamp_utf8_index(text, utf8_offset)
    }

    fn utf8_to_utf16_in_text(text: &str, utf8_offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        let clamped_utf8 = Self::clamp_utf8_index(text, utf8_offset);

        for ch in text.chars() {
            if utf8_count >= clamped_utf8 {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_from_utf16_for_text(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
        let start = Self::utf16_to_utf8_in_text(text, range_utf16.start);
        let end = Self::utf16_to_utf8_in_text(text, range_utf16.end);
        if end < start { end..start } else { start..end }
    }

    fn range_to_utf16_for_text(text: &str, range_utf8: &Range<usize>) -> Range<usize> {
        let start = Self::utf8_to_utf16_in_text(text, range_utf8.start);
        let end = Self::utf8_to_utf16_in_text(text, range_utf8.end);
        if end < start { end..start } else { start..end }
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        Self::range_from_utf16_for_text(&self.text, range_utf16)
    }

    fn range_to_utf16(&self, range_utf8: &Range<usize>) -> Range<usize> {
        Self::range_to_utf16_for_text(&self.text, range_utf8)
    }

    fn replacement_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range())
    }

    pub(super) fn text_for_range(
        &self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
    ) -> String {
        let range = self.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.range_to_utf16(&range));
        self.text[range].to_string()
    }

    pub(super) fn selected_text_range(&self) -> UTF16Selection {
        UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        }
    }

    pub(super) fn marked_text_range(&self) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    pub(super) fn unmark_text(&mut self) {
        self.marked_range = None;
    }

    pub(super) fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        let range = self.replacement_range(range_utf16);
        self.text.replace_range(range.clone(), text);
        let cursor = range.start + text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
    }

    pub(super) fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) {
        let range = self.replacement_range(range_utf16);
        self.text.replace_range(range.clone(), new_text);

        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(range.start..range.start + new_text.len());
        }

        self.selection_reversed = false;
        if let Some(local_selected_utf16) = new_selected_range_utf16 {
            let local_selected = Self::range_from_utf16_for_text(new_text, &local_selected_utf16);
            let selected_start = range.start + local_selected.start;
            let selected_end = range.start + local_selected.end;
            self.selected_range = selected_start..selected_end;
        } else {
            let cursor = range.start + new_text.len();
            self.selected_range = cursor..cursor;
        }
    }
}

pub(super) struct InlineInputElement {
    view: Entity<TerminalView>,
    focus_handle: FocusHandle,
}

impl InlineInputElement {
    pub(super) fn new(view: Entity<TerminalView>, focus_handle: FocusHandle) -> Self {
        Self { view, focus_handle }
    }
}

impl IntoElement for InlineInputElement {
    type Element = gpui::Canvas<()>;

    fn into_element(self) -> Self::Element {
        let focus_handle = self.focus_handle;
        let view = self.view;

        canvas(
            |_bounds, _window, _cx| (),
            move |bounds, _, window, cx| {
                window.handle_input(
                    &focus_handle,
                    ElementInputHandler::new(bounds, view.clone()),
                    cx,
                );
            },
        )
        .size_full()
    }
}

impl TerminalView {
    pub(super) fn sync_inline_input_target(&mut self) {
        self.inline_input_target = if self.command_palette_open {
            Some(InlineInputTarget::CommandPalette)
        } else if self.renaming_tab.is_some() {
            Some(InlineInputTarget::TabRename)
        } else {
            None
        };
    }

    pub(super) fn active_inline_input_target(&self) -> Option<InlineInputTarget> {
        match self.inline_input_target {
            Some(InlineInputTarget::CommandPalette) if self.command_palette_open => {
                Some(InlineInputTarget::CommandPalette)
            }
            Some(InlineInputTarget::TabRename) if self.renaming_tab.is_some() => {
                Some(InlineInputTarget::TabRename)
            }
            _ => None,
        }
    }

    fn active_inline_input_state(&self) -> Option<&InlineInputState> {
        match self.active_inline_input_target()? {
            InlineInputTarget::CommandPalette => Some(&self.command_palette_input),
            InlineInputTarget::TabRename => Some(&self.rename_input),
        }
    }

    fn active_inline_input_state_mut(&mut self) -> Option<&mut InlineInputState> {
        match self.active_inline_input_target()? {
            InlineInputTarget::CommandPalette => Some(&mut self.command_palette_input),
            InlineInputTarget::TabRename => Some(&mut self.rename_input),
        }
    }

    pub(super) fn command_palette_query(&self) -> &str {
        self.command_palette_input.text()
    }

    pub(super) fn command_palette_query_changed(&mut self, cx: &mut Context<Self>) {
        let len = self.filtered_command_palette_items().len();
        self.clamp_command_palette_selection(len);
        if len > 0 {
            self.command_palette_scroll_handle
                .scroll_to_item(self.command_palette_selected, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn enforce_tab_rename_limit(&mut self) {
        let current_chars = self.rename_input.text().chars().count();
        if current_chars <= MAX_TAB_TITLE_CHARS {
            return;
        }

        let truncated: String = self
            .rename_input
            .text()
            .chars()
            .take(MAX_TAB_TITLE_CHARS)
            .collect();
        self.rename_input.set_text(truncated);
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let state = self.active_inline_input_state()?;
        Some(state.text_for_range(range, adjusted_range))
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let state = self.active_inline_input_state()?;
        Some(state.selected_text_range())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let state = self.active_inline_input_state()?;
        state.marked_text_range()
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(state) = self.active_inline_input_state_mut() {
            state.unmark_text();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.active_inline_input_target() else {
            return;
        };

        let Some(state) = self.active_inline_input_state_mut() else {
            return;
        };
        state.replace_text_in_range(range, text);

        match target {
            InlineInputTarget::CommandPalette => self.command_palette_query_changed(cx),
            InlineInputTarget::TabRename => {
                self.enforce_tab_rename_limit();
                cx.notify();
            }
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.active_inline_input_target() else {
            return;
        };

        let Some(state) = self.active_inline_input_state_mut() else {
            return;
        };
        state.replace_and_mark_text_in_range(range, new_text, new_selected_range);

        match target {
            InlineInputTarget::CommandPalette => self.command_palette_query_changed(cx),
            InlineInputTarget::TabRename => {
                self.enforce_tab_rename_limit();
                cx.notify();
            }
        }
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.active_inline_input_target().map(|_| element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let state = self.active_inline_input_state()?;
        Some(state.selected_text_range().range.start)
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.active_inline_input_target().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_range_conversion_handles_multibyte_text() {
        let state = InlineInputState::new("a😄é".to_string());
        let utf16 = state.range_to_utf16(&(1..7));
        assert_eq!(utf16, 1..4);
        let utf8 = state.range_from_utf16(&utf16);
        assert_eq!(utf8, 1..7);
    }

    #[test]
    fn replace_text_uses_selection_when_no_range() {
        let mut state = InlineInputState::new("hello".to_string());
        state.selected_range = 1..4;
        state.replace_text_in_range(None, "i");
        assert_eq!(state.text(), "hio");
        assert_eq!(state.selected_range(), 2..2);
    }

    #[test]
    fn replace_and_mark_sets_marked_and_selection() {
        let mut state = InlineInputState::new("abcd".to_string());
        state.selected_range = 1..3;
        state.replace_and_mark_text_in_range(Some(1..3), "xy", Some(0..1));
        assert_eq!(state.text(), "axyd");
        assert_eq!(state.marked_range, Some(1..3));
        assert_eq!(state.selected_range(), 1..2);
    }

    #[test]
    fn unmark_clears_marked_range() {
        let mut state = InlineInputState::new("abc".to_string());
        state.marked_range = Some(0..2);
        state.unmark_text();
        assert_eq!(state.marked_range, None);
    }

    #[test]
    fn text_with_cursor_inserts_cursor_for_collapsed_selection() {
        let mut state = InlineInputState::new("termy".to_string());
        state.selected_range = 2..2;
        assert_eq!(state.text_with_cursor(), "te▌rmy");
    }
}
