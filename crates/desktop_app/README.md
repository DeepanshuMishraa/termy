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

## Validation

```sh
cargo test -p termy
cargo check -p termy
```

## Forbidden Dependencies

- `termy_ffi`
- native host app packages
- website packages
