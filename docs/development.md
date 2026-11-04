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
sudo apt install build-essential cmake pkg-config clang libclang-dev libpipewire-0.3-dev libspa-0.2-dev libsndfile1
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
and exits successfully, including while the main window is closed to the tray.
Quitting the app or terminating its process releases the
lock automatically, including after a crash. The lock file stays on disk and must
not be deleted while the app is running. Locking uses Rust's standard library on
Unix and Windows and does not require D-Bus. Filesystem or locking errors stop
startup and report the affected path.

## Application logs

After acquiring the instance lock, `src/main.rs` initializes `src/logging.rs`
using `Store::logs_directory()`, before starting Iced and its workers. The
`log` facade and `flexi_logger` write to `~/.raspiboard/logs/raspiboard_rCURRENT.log`.
Daily rotation and seven-day archive retention are handled by the logger;
restarting also archives the previous session. Cleanup runs at startup and on
rotation, using each archive's modification time. A second instance exits before
initializing logging, so it cannot rotate the running instance's log.

Lines use local time with milliseconds:

```text
[2026-11-03 09:30:00.123] raspiboard::audio.ERROR: Playback failed
```

The default filter records application messages at INFO and above, and dependency
messages at WARN and above. Warnings and errors also appear on stderr. Startup,
shutdown, audio connection and recording state changes, storage failures,
waveform failures, hotkey and tray errors, and panics are logged. The panic hook
also preserves Rust's usual stderr report. `RUST_LOG` overrides the default filter;
to include audio control commands and other debug diagnostics:

```sh
RUST_LOG=warn,raspiboard=debug cargo run --locked
```

Logging tests use isolated directories inside `test-results/logging/` and cover
formatting, filtering, restart preservation, rotation, age-based cleanup,
unusable directories, and panic reporting without starting the GUI.

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

The GitHub workflow uses one build/test/package job. The packaging command runs
the full application test suite with `cargo test --release --locked --target
x86_64-unknown-linux-gnu`, formatting, Clippy, and PHP packaging checks once,
then generates both formats from the same release executable. Debian 13 and
Ubuntu 26.04 package validation must pass before upload. Cargo downloads and
compiled outputs, plus Docker tooling layers, are cached between runs.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Automated tests cover concurrent playback requests, group cancellation and transport,
application passthrough discovery and endpoint loss, bounded live capture,
remote gain, circular capture buffers, and active blink redraws,
recording encoders for FLAC/WAV/Ogg, recording-folder preferences, writer draining,
silence preservation, queue faults and independent recording transport,
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
Regressions cover MP3 crop sample equality without reservoir underflow warnings,
stale ports and links after node removal/reuse, and benign resource-removal errors
that must not restart the audio worker. The MP3 seek fixture is generated with
the command in `tests/fixtures/audio/README.md`.
Seeking tests cover the strict five-second crop threshold, overlap/preview
exclusion, crop bounds, backward and forward jumps, generation rejection, pause
preservation, silent paused starts, independent duration analysis, and pointer
drag/release/cancellation without opening a window.
Title bar tests exercise native widget event handling without opening a window:
search and control click capture, double-click maximize, all eight resize
directions, maximized resize suppression, and stale window-state callbacks.
Search tests cover label matching, clearing/no matches, unchanged playback and
hotkeys, transient-menu cancellation, and reordering around hidden pads.
An ignored headless timing test accepts a local JSON array containing `path`,
`extension`, `start_ms` and `end_ms` entries. Keep that fixture under test-results
and run `RASPIBOARD_PROFILE_INPUT=test-results/recording/performance-input.json
cargo test --locked --lib profile_audio_preparation_and_waveform -- --ignored
--nocapture`. It measures waveform/crop preparation, verifies cached reuse and
compares seeked samples against linear decoding without playing audio.
Single-instance tests cover competing owners, leftover lock files, filesystem
errors, and lock release after a separate owner process is terminated.
Hotkey tests cover session membership, held-key suppression, assignment and Clear,
stale callbacks, deletion, cancellation, errors, and restoring saved shortcuts.

Follow the [manual acceptance checks](acceptance.md) to verify live routing.
Applications may publish several recording streams. Both selectors prefer
`media.title`, then `media.name`, trimmed and shown in brackets after the
application name. Missing or duplicate labels fall back to stream IDs within each
selector. Labels refresh with discovery metadata; routing retains the node/serial
identity. Headless graph tests cover both selectors, title preference, blank
metadata, duplicate and numeric-title collisions, and title updates. Streams whose
upstream source cannot be identified remain selectable; the dropdown omits source
descriptions. Known desktop-audio-only captures are excluded. The receiving
application's mute, push-to-talk, and noise suppression settings still apply.

## Code walkthroughs

- [Interface and widgets](learning/01-interface.md)
- [Audio routing and playback](learning/02-audio-routing.md)
- [Persistence and background saves](learning/03-persistence.md)
- [Saved data format](saved-data.md)
- [Linux packaging](packaging.md)
- [Bundled fonts and their licenses](../assets/fonts/README.md)
- [Vendored Wayland icon support](../vendor/winit/PATCHES.md)

## Tray lifecycle

`src/ui/tray.rs` exposes a ksni StatusNotifierItem through an iced subscription,
with the embedded application icon and Show/Quit actions. ksni uses the existing
async-io runtime family and requires no GTK or AppIndicator native libraries.
`src/ui/windows.rs` disables automatic close on the main window; close requests
pass through the UI message handler. When close-to-tray is enabled and the tray
host is available, the native windows close while the daemon, audio, hotkeys, and
persistence writer remain alive. Show creates a new main window using the retained
board state. Closing and recreating works on Wayland, where winit cannot hide an
existing window. Loss of the tray host reopens the main window automatically.
Primary icon activation sends a separate Toggle event. For an existing window,
Iced queries its minimized state: minimized windows restore, visible windows use
the shared hide-to-tray path. Replies apply only to the current main window while
the tray is available. This explicit action works independently of the close
preference; the Show menu action always restores or focuses the board.

The automated suite covers enabled/disabled close behavior, missing tray support,
restoring the board, tray click toggling, stale window-state replies, tray host
loss, explicit Quit, and preference persistence.
Live tray rendering and desktop integration are covered in the manual acceptance
checks.

## Main window header

`src/ui/windows.rs` opens the main window without system decorations; Settings
and Sound options retain them. `src/ui/title_bar.rs` composes the branded header,
centered search input, window buttons, and resize border from Iced widgets.
Iced's image feature displays the embedded PNG and its native window tasks
perform move, resize, minimize, and maximize. Resize events query confirmed
maximize state; replies are accepted only for the current main window ID.
The main native window supports transparency, with a transparent application
clear color behind an opaque rounded frame. Auxiliary views paint their own
opaque background. Frame radius and border come from the theme's panel tokens;
maximized windows use square corners. Circular header controls reuse the raised
pad renderer's bevel, shadow and animation with a circular radius.

`src/ui/search.rs` filters the rendered grid using temporary query state and
each pad's displayed label. Playback, hotkeys, persistence, and reordering still
use the complete pad collection. Header Close enters the existing tray/quit
path rather than closing the native window directly.
