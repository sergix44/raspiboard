# Local patches to winit

This directory contains winit **0.30.13** from crates.io, based on upstream commit
`e9809ef54b18499bb4f2cac945719ecc2a61061b`. The upstream Apache-2.0 license is
preserved in `LICENSE`. The application selects this copy with its root
`[patch.crates-io]` entry; the package version stays unchanged.

## Native Wayland window icons

Reference: upstream native icon support at
[`e1ad70bfed1936fb615d602ade72a617cb9bff4a`](https://github.com/rust-windowing/winit/commit/e1ad70bfed1936fb615d602ade72a617cb9bff4a).
This backport adapts the `xdg-toplevel-icon-v1` integration to the 0.30 API and
adds pixel conversion and explicit buffer lifetime management.

- `wayland/state.rs` optionally binds the icon manager. Compositors without the
  protocol retain their existing behavior.
- `wayland/window/mod.rs` forwards the initial window icon and later
  `Window::set_window_icon` calls to `wayland/window/state.rs`. Passing `None`
  resets the native icon.
- `wayland/types/xdg_toplevel_icon.rs` sends the icon requests and retains the
  shared-memory buffer until after its protocol icon is destroyed. Replacement
  allocates separate storage before retiring the previous icon. The protocol
  does not send `wl_buffer.release` for icon buffers.
- `wayland/types/icon_pixels.rs` converts RGBA pixels to native-endian,
  premultiplied ARGB8888, pads rectangular images to transparent squares, and
  rejects dimensions whose stride or padded allocation exceeds signed 32-bit
  shared-memory limits.
- `src/window.rs` documents Wayland icon support and the compositor requirement.

Run the vendored unit tests from the project root:

```sh
cargo test --manifest-path vendor/winit/Cargo.toml --lib
```

This command needs the upstream development dependencies available in Cargo's
cache or network access. The headless Unix-socket protocol test checks setting,
replacing, and clearing icons, request ordering, pixel contents, and buffer
destruction. Pixel tests also cover transparency, premultiplication, rectangular
padding, and shared-memory dimension boundaries. No compositor or GUI is started.

Actual taskbar presentation remains compositor-dependent and requires a manual
desktop check on a compositor advertising `xdg_toplevel_icon_manager_v1`.
