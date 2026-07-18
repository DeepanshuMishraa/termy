# termy

Main desktop application.

## Owner

This crate owns the GPUI app shell, windows, titlebar/chrome, menus, settings, onboarding, command execution, and user-visible desktop workflows.

Important internal areas:

- `src/terminal_view/`: terminal surface, tabs, panes, search, command palette, input, rendering, persistence, and runtime coordination.
- `src/settings_view/`: settings UI and state application.
- `src/onboarding/`: first-run and import flows.
- `src/config/`: app-owned config I/O and mutation.

Push reusable headless behavior into `termy_core` or a pure domain crate. Push GPUI-adjacent terminal adapter behavior into `termy_terminal_ui` only when it is reusable outside the desktop app shell.

## Kitty graphics

The terminal surface renders static images sent through the Kitty graphics
protocol. The shared terminal core handles APC parsing, direct and file-backed
transfers, chunking, PNG/RGB/RGBA data, zlib compression, placements, deletion,
quiet-mode replies, source rectangles, cursor movement, and storage limits. The
desktop renderer handles clipping, cell/pixel sizing, z-index ordering, and GPU
image caching. Animations, shared-memory transfers, Unicode placeholders, and
relative placements are not currently supported.

Natural-size placements (no `c`/`r`) use 1:1 image pixels and **truncate** on
the right edge of the screen from the cursor, matching Kitty — they do not
scale-to-fit. Explicit `c`/`r` (used by clients such as Grok Build’s image
preview via `fit_image_to_cells`) scale the image into that cell rectangle.
PTY `TIOCGWINSZ` pixel metrics are derived from cell size and never report zero,
so clients can size placements correctly.

## Validation

```sh
cargo test -p termy
cargo check -p termy
```

## Forbidden Dependencies

- `termy_ffi`
- native host app packages
- website packages
