# Context

## Rounded box-corner rendering fix

The native macOS renderer had visible kinks/notches in Unicode rounded box
corners such as `╭`, `╮`, `╯`, and `╰`. The issue was in the Swift renderer's
custom stroked path for rounded box-drawing glyphs.

Changes made:

- Added `TerminalRoundedCornerPathSpec` and
  `TerminalBoxDrawing.roundedCornerPathSpec(...)` in
  `macos/Sources/TermySwift/Views/TerminalBoxDrawing.swift`.
- Switched `TerminalGridNSView.strokeRoundedCorner(...)` in
  `macos/Sources/TermySwift/Views/TerminalGridView.swift` to draw from the
  shared path spec instead of constructing the curve inline.
- Used the standard cubic quarter-circle control distance
  `radius * 0.5522847498307936` so the curve transitions smoothly into the
  straight stubs.
- Added focused tests in
  `macos/Tests/TermySwiftTests/TerminalBoxDrawingTests.swift` for smooth
  top-left corner geometry and endpoint alignment for all four rounded corners.

Validation run:

```sh
TERMY_FFI_LIBRARY_PATH="$PWD/target/debug" swift test --package-path macos --filter TerminalBoxDrawingTests
swift build --package-path macos -Xswiftc -warnings-as-errors
```

Both commands passed.

No changes were needed in `crates/ffi` or `crates/core`; the bug was isolated to
the native macOS box-drawing renderer.
