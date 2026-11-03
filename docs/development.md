# Development

Run the commands below from the repository root. For installation and everyday
use, start with the [README](../README.md).

## Build and run

Install a current stable Rust toolchain with Cargo. The project uses Rust's 2024
edition and iced 0.14. Development checks currently use Rust 1.97.0. PipeWire
0.3.65 or later must be running in your desktop session, with a session manager
such as WirePlumber.

Install the native build dependencies on Debian or Ubuntu:

```sh
sudo apt install build-essential cmake pkg-config clang libclang-dev libpipewire-0.3-dev libspa-0.2-dev
```

The file picker uses the desktop's file dialog through XDG Desktop Portal. Use
your desktop's portal backend (normally installed with GNOME or KDE).
Global hotkeys also require a backend that implements the Global Shortcuts portal,
such as KDE's backend. A backend with only file dialogs cannot provide global keys.

From the project directory:

```sh
cargo build --locked
cargo run --locked
```

Run the application from your graphical desktop session. iced supports both
Wayland and X11 with its default features. The first build downloads and compiles
the framework and its dependencies, so it takes longer than subsequent builds.

Raspiboard allows one running instance per user data directory. Before opening a
window or starting workers, it takes an exclusive operating-system file lock on
`~/.raspiboard/instance.lock`. A second launch prints `Raspiboard is already running.`
and exits successfully. Closing the app or terminating its process releases the
lock automatically, including after a crash. The lock file stays on disk and must
not be deleted while the app is running. Locking uses Rust's standard library on
Unix and Windows and does not require D-Bus. Filesystem or locking errors stop
startup and report the affected path.

## Desktop launcher and application icon

Application windows use `img/icon_full.png`, embedded in the binary and decoded
once into a 256×256 window icon. They identify as `io.raspiboard.Raspiboard` on
Linux. X11 and Wayland desktops supporting the XDG toplevel icon protocol can
display the embedded window icon directly. Other Wayland desktops resolve the
icon through the matching installed desktop launcher. A launcher also adds the
application menu entry.

To install the release binary and launcher for your user account (requires
`desktop-file-utils` for `desktop-file-install`):

```sh
cargo build --release --locked
install -Dm755 target/release/raspiboard "$HOME/.local/bin/raspiboard"
install -Dm644 img/icon_full.png "$HOME/.local/share/icons/io.raspiboard.Raspiboard.png"
desktop-file-install --dir="$HOME/.local/share/applications" \
  --set-key=Exec --set-value="$HOME/.local/bin/raspiboard" \
  assets/io.raspiboard.Raspiboard.desktop
```

Release builds use optimization level 3, Thin LTO and one code-generation unit.
Line tables are retained for crash reports, and normal panic cleanup remains
enabled. CPU-specific instructions are not forced, keeping the binary suitable
for compatible machines of the same target architecture. A compatible Linux and
PipeWire runtime is still required. These settings favor runtime optimization over
build speed; actual performance still needs measurement.

For the first release test without installing a launcher:

```sh
cargo build --release --locked -j 2
RUST_BACKTRACE=1 ./target/release/raspiboard
```

The two-job limit reduces peak compilation memory use. To run the automated
suite with release optimizations, use `cargo test --release --locked -j 2`.

Launch Raspiboard from the application menu. X11 window decorations can also
use the embedded icon directly; its placement depends on the window manager.

For development with `cargo run`, register a launcher pointing to this checkout
instead of installing a release binary:

```sh
cargo build --locked
project_root=$(pwd -P)
desktop-file-install --dir="$HOME/.local/share/applications" \
  --set-key=Exec --set-value="\"$project_root/target/debug/raspiboard\"" \
  --set-key=Icon --set-value="$project_root/img/icon_full.png" \
  assets/io.raspiboard.Raspiboard.desktop
```

On KDE Plasma, refresh the application cache with
`XDG_MENU_PREFIX=plasma- XDG_CURRENT_DESKTOP=KDE kbuildsycoca6 --noincremental`,
then close and reopen Raspiboard. The launcher filename must match the app ID;
placing a `.desktop` file only in the repository does not register it with
Wayland. Repeat registration if the checkout moves. Installing the release
launcher above replaces this development launcher.

Opus decoding uses libopus, compiled and bundled into the binary by the Rust
dependency. Building it requires CMake and the C toolchain listed above.

## Checks

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Automated tests cover concurrent playback requests, group cancellation and transport,
overlap persistence, crop boundaries and gain, bounded waveform summaries,
waveform selection, stale analysis results, color/reset persistence, microphone-path discovery, channel mapping, reused stream
IDs, stale playback commands, Audio decoding into both outputs, independent gains,
volume linking, file-assignment state, cancellation, and buffer exhaustion. A
theme-selection check covers immediate updates, reopening Settings, and restoring
the default; another checks that theme changes preserve playback state. A
sanitized PipeWire graph fixture also covers capture through an effects chain.
Headless widget tests cover the animated sound button's click cancellation,
disabled state, right-click actions, and redraws ending when motion settles.
Persistence tests use isolated directories inside `test-results/` and cover
settings/button round trips, grid ordering, malformed-file preservation, atomic
writes, shutdown flushing, and independent save-error recovery. They also check
that restoring saved data does not start playback or select a recording target.
These tests do not connect to a live audio server or open a window.
Single-instance tests cover competing owners, leftover lock files, filesystem
errors, and lock release after a separate owner process is terminated.
Hotkey tests cover session membership, held-key suppression, assignment and Clear,
stale callbacks, deletion, cancellation, errors, and restoring saved shortcuts.

Follow the [manual acceptance checks](acceptance.md) to verify live routing.
Applications may publish several recording streams; the destination list includes
stream IDs to distinguish them. Streams whose upstream source cannot be identified
remain selectable; the dropdown omits source descriptions. Known desktop-audio-only captures
are excluded. The receiving application's mute, push-to-talk, and noise suppression
settings still apply.

## Code walkthroughs

- [Interface and widgets](learning/01-interface.md)
- [Audio routing and playback](learning/02-audio-routing.md)
- [Persistence and background saves](learning/03-persistence.md)
- [Saved data format](saved-data.md)
- [Linux packaging](packaging.md)
- [Bundled fonts and their licenses](../assets/fonts/README.md)
- [Vendored Wayland icon support](../vendor/winit/PATCHES.md)
