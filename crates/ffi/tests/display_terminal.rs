//! Verifies the display-only terminal (no PTY) used for tmux control-mode panes:
//! bytes fed via `termy_terminal_feed_output` land in the grid and are readable
//! through the normal snapshot/render FFI.

use termy_ffi::{
    TermyFfiFrame, TermyFfiKittyGraphicsBatch, TermyFfiStatus, TermyFfiTerminal,
    termy_display_terminal_new, termy_frame_free, termy_kitty_graphics_batch_free,
    termy_size_default, termy_terminal_feed_output, termy_terminal_free,
    termy_terminal_kitty_graphics_placements, termy_terminal_kitty_graphics_revision,
    termy_terminal_snapshot,
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
fn display_terminal_exposes_kitty_graphics_placements() {
    let size = termy_size_default();
    let mut terminal: *mut TermyFfiTerminal = std::ptr::null_mut();
    assert_eq!(
        unsafe { termy_display_terminal_new(size, &mut terminal) },
        TermyFfiStatus::Ok
    );

    let command = b"\x1b_Ga=T,f=32,s=1,v=1,i=77,c=2,r=3;AQID/w==\x1b\\";
    assert_eq!(
        unsafe { termy_terminal_feed_output(terminal, command.as_ptr(), command.len()) },
        TermyFfiStatus::Ok
    );

    let mut revision = 0;
    assert_eq!(
        unsafe { termy_terminal_kitty_graphics_revision(terminal, &mut revision) },
        TermyFfiStatus::Ok
    );
    assert!(revision > 0);

    let mut batch = TermyFfiKittyGraphicsBatch::default();
    assert_eq!(
        unsafe { termy_terminal_kitty_graphics_placements(terminal, &mut batch) },
        TermyFfiStatus::Ok
    );
    assert_eq!(batch.revision, revision);
    let placements =
        unsafe { std::slice::from_raw_parts(batch.placements_ptr, batch.placements_len) };
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].image_id, 77);
    assert!(placements[0].has_display_cols);
    assert_eq!(placements[0].display_cols, 2);
    let png = unsafe { std::slice::from_raw_parts(placements[0].png.ptr, placements[0].png.len) };
    assert!(png.starts_with(b"\x89PNG"));

    assert_eq!(
        unsafe { termy_kitty_graphics_batch_free(&mut batch) },
        TermyFfiStatus::Ok
    );
    assert_eq!(unsafe { termy_terminal_free(terminal) }, TermyFfiStatus::Ok);
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
