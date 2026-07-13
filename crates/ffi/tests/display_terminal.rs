//! Verifies the display-only terminal (no PTY) used for tmux control-mode panes:
//! bytes fed via `termy_terminal_feed_output` land in the grid and are readable
//! through the normal snapshot/render FFI.

use termy_ffi::{
    TermyFfiFrame, TermyFfiStatus, TermyFfiTerminal, termy_display_terminal_new, termy_frame_free,
    termy_size_default, termy_terminal_feed_output, termy_terminal_free, termy_terminal_snapshot,
};

#[test]
fn display_terminal_feed_output_lands_in_grid() {
    let size = termy_size_default();
    let mut terminal: *mut TermyFfiTerminal = std::ptr::null_mut();
    assert_eq!(
        unsafe { termy_display_terminal_new(size, &mut terminal) },
        TermyFfiStatus::Ok
    );
    assert!(!terminal.is_null());

    let text = b"hello world";
    assert_eq!(
        unsafe { termy_terminal_feed_output(terminal, text.as_ptr(), text.len()) },
        TermyFfiStatus::Ok
    );

    let mut frame: TermyFfiFrame = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { termy_terminal_snapshot(terminal, &mut frame) },
        TermyFfiStatus::Ok
    );

    let cells = unsafe { std::slice::from_raw_parts(frame.cells_ptr, frame.cells_len) };
    let rendered: String = cells
        .iter()
        .filter(|cell| cell.render_text)
        .filter_map(|cell| char::from_u32(cell.codepoint))
        .collect();
    assert!(
        rendered.contains("hello world"),
        "expected fed text in grid, got: {rendered:?}"
    );

    unsafe { termy_frame_free(&mut frame) };
    unsafe { termy_terminal_free(terminal) };
}

#[test]
fn display_terminal_write_is_noop_without_pty() {
    // A display terminal has no PTY; write must not panic or send anywhere.
    let size = termy_size_default();
    let mut terminal: *mut TermyFfiTerminal = std::ptr::null_mut();
    assert_eq!(
        unsafe { termy_display_terminal_new(size, &mut terminal) },
        TermyFfiStatus::Ok
    );
    let input = b"ignored";
    assert_eq!(
        unsafe { termy_ffi::termy_terminal_write(terminal, input.as_ptr(), input.len()) },
        TermyFfiStatus::Ok
    );
    unsafe { termy_terminal_free(terminal) };
}

#[test]
fn display_terminal_marks_wide_character_spacer_cells() {
    let size = termy_size_default();
    let mut terminal: *mut TermyFfiTerminal = std::ptr::null_mut();
    assert_eq!(
        unsafe { termy_display_terminal_new(size, &mut terminal) },
        TermyFfiStatus::Ok
    );

    let text = "A界B".as_bytes();
    assert_eq!(
        unsafe { termy_terminal_feed_output(terminal, text.as_ptr(), text.len()) },
        TermyFfiStatus::Ok
    );
    let mut frame: TermyFfiFrame = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { termy_terminal_snapshot(terminal, &mut frame) },
        TermyFfiStatus::Ok
    );

    let cells = unsafe { std::slice::from_raw_parts(frame.cells_ptr, frame.cells_len) };
    assert_eq!(cells[0].codepoint, 'A' as u32);
    assert_eq!(cells[1].codepoint, '界' as u32);
    assert!(cells[1].render_text);
    assert!(cells[2].wide_character_spacer);
    assert!(!cells[2].render_text);
    assert_eq!(cells[3].codepoint, 'B' as u32);

    unsafe { termy_frame_free(&mut frame) };
    unsafe { termy_terminal_free(terminal) };
}
