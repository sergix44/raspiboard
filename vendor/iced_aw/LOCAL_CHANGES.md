# Local iced_aw changes

Source: the published `iced_aw` 0.14.1 crate, licensed under MIT. Upstream
copyright and license are preserved in `LICENSE` and the original source.

The picker extension changes two runtime source files:

- `src/widget/color_picker.rs`: adds `ColorPicker::actions(cancel, submit)`
  for arbitrary action elements, and retains those elements in the picker tree
  across rebuilds and visibility changes. Includes three regression tests using
  wrapped buttons for cancellation, current-color submission after rebuilding,
  and redraw/event forwarding before submission.
- `src/widget/overlay/color_picker.rs`: borrows the same stored action elements
  for layout, drawing, events, operations, and mouse interaction. Default icon
  buttons retain their original appearance. Submit messages are mapped to the
  current selected color through `Shell::merge`, preserving redraw requests,
  event capture, and layout/widget invalidation. Cancel mouse interaction uses
  the cancel action's own state tree.

A third source adjustment in `src/lib.rs` replaces the font generator with the
same six basic/advanced glyph helpers from `font.ttf`. The upstream procedural
macro reads a literal relative path from rustc's working directory, which fails
when this crate is built as a local path dependency. Fixed glyph helpers preserve
the font API and work from either manifest directory.

The bundled manifest omits examples, integration-test targets, and development
dependencies because those upstream targets are not shipped here. Sources,
`font.ttf`, `README.md`, and `LICENSE` are otherwise copied; example assets and
images are omitted.

Both custom actions must publish a message when activated. Cancel messages pass
through unchanged. Any submit message invokes the picker's existing `on_submit`
callback with the current color, so the cancel message can be reused by the
submit widget.

Run the picker tests without opening a window:

```sh
cargo test --manifest-path vendor/iced_aw/Cargo.toml --no-default-features --features color_picker color_picker
```

Remove this directory and the application's path dependency when an upstream
release supports arbitrary picker actions while retaining their widget state
and forwarding submit-shell redraw/event requests. Replace `.actions(...)`
with that release's equivalent API if necessary.
