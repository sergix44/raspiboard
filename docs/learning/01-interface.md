# The soundboard interface

`src/main.rs` starts the application; `src/ui.rs` owns its interface and state.
The [audio walkthrough](02-audio-routing.md) explains the worker and connections.

## Cargo and the entry point

`Cargo.toml` names the executable, selects Rust's 2024 edition, and declares
dependencies. Cargo is Rust's build, dependency, and test tool; a reusable Rust
library is called a crate. `Cargo.lock` records the exact selected versions.
`--locked` makes Cargo refuse a build that would change that file.

Rust calls `main` when the program starts. `mod ui;` loads `ui.rs`, and
`use ui::Soundboard;` makes its type available without repeating the module path.
`pub` makes an item accessible outside its module.

`iced::daemon` receives three functions: `Soundboard::start` restores state and
opens the main window, `Soundboard::update` handles events, and
`Soundboard::view` describes the requested window's interface.
`src/ui/windows.rs` sets the main window's 1100×680 initial size and 1000×360
minimum. The chained configuration sets per-window titles, the shared theme
callback, and subscriptions. It also registers the bundled
Lucide icon font before the first window is drawn and sets `Font::MONOSPACE` as
the default font for interface text. `Soundboard::theme` returns
a cached custom Iced theme from the saved `ThemeChoice`. Only Deck and Console
are exposed; Deck is the default and Iced/desktop theme overrides do not select
an alternate palette.
`run()` starts iced's event loop. `main` returns `iced::Result`, allowing Rust to
report a startup or runtime error.

## State and events

`Soundboard` is a struct grouping the two volume percentages, their link state,
the saved button records, active button ID, file-dialog state, audio controller,
worker snapshot, and errors.
It also owns the application theme choice and the main and Settings window IDs.
Theme, volumes, linking, and button assignments are saved through
`src/ui/persistence.rs`; see the [persistence walkthrough](03-persistence.md).
Each button's `Option<PathBuf>` holds either an assigned file (`Some(path)`) or
no file (`None`). The picker records the button ID while it is open.
The volume fields use `u8`, an unsigned integer with values from 0 through 255;
the sliders and update handlers limit levels to 0 through 100.

`impl Default for Soundboard` implements the `Default` trait, constructing state
with both volumes at 100%, unlinked, and one empty sound button. This constructor
does not touch disk and remains useful in tests. The app uses `boot` instead,
which loads the saved files and starts the writer. Recording target selection,
playback, picker state, and Settings visibility start fresh.
`Self` means the type being implemented, here `Soundboard`.

`Message` is an enum describing the events the interface can handle:

```rust
RemoteVolumeChanged(u8),
LocalVolumeChanged(u8),
ToggleVolumeLink,
OpenButtonMenu(String),
OpenButtonOptions(String),
OpenAudio(String),
DeleteButton(String),
AudioChosen { id: String, path: Option<PathBuf> },
PlayButton(String),
Play,
Stop,
OpenSettings,
CloseSettings,
ThemeChanged(ThemeChoice),
```

Other variants carry audio events and target selections. An enum represents one
of a fixed set of alternatives, optionally with data. `derive(Debug, Clone)` asks
Rust to generate debug formatting and cloning. An exhaustive `match` in `update`
ensures that adding an event requires deciding how to handle it.

## Updates and asynchronous tasks

`update(&mut self, message: Message)` borrows state exclusively so it can change
it, then returns `Task<Message>`. Ordinary updates return `Task::none()`.

Moving a slider changes its stored percentage. When linked, the handler copies
that value to the other percentage. Enabling the link copies Remote to Local;
disabling it retains both. `send_volumes` sends the pair to the worker, including
when the worker first becomes ready, so earlier slider edits are preserved.
Durable volume/link changes also queue a settings save.

The sound uses iced's `mouse_area(...).on_right_press(Message::OpenButtonMenu(id))` around
the button. This catches right-click even when left-click playback is disabled.
Choosing Replace sound sends `OpenAudio`, which ignores repeated requests while a picker is open, stops playback, and
returns `Task::perform` around the existing `rfd::AsyncFileDialog`. The task awaits
the desktop's native file dialog without blocking interface updates.

When the dialog completes, `AudioChosen` checks the original button ID and replaces
only that button's assignment, then queues its JSON save. An unrelated stale
picker result is ignored. Cancel leaves the previous file and saved metadata
alone. Assignment does not start playback.

`src/ui/grid_menu.rs` wraps the scrollable grid in a `mouse_area`. Right-clicking
empty space opens a menu at the recorded pointer position. Existing pads capture
their own right-clicks first and open Options, Replace sound, and Delete. Native iced `float`
and `stack` widgets place the menu within the viewport; an opaque click layer
prevents outside clicks from activating controls below it. Escape dismisses it.

The menu uses a 152-pixel panel with 2-pixel padding and a 28-pixel flat action
row. Its surface and hover colors reuse iced's native dropdown menu palette;
a thin border and subtle shadow separate it from the grid. The row's text is
left-aligned at 12 pixels. It uses no raised pad wrapper or lift animation.

`AddSound` closes the menu and generates a UUIDv7 for the pending new button.
Both new-button and replacement requests use `choose_wav`, which stops playback
and starts the same asynchronous picker. No new pad is inserted until a matching
`AudioChosen` supplies a file. An ID already in the grid updates that pad; a pending
new ID inserts a pad after the default category's entries, assigns the next
`sorting` value, selects it for footer Play, and queues `Change::AddButton`.
Cancel leaves the grid and active pad unchanged. Only one picker can be pending.

The `playback_for` helper resolves a specific button's path and the optional
selected target only when connected and no picker is open. No target means
local-only playback. Its result controls whether
that pad can play and supplies the playback request. Footer Play uses the active
button; clicking a pad or opening its picker selects it. `play_command` captures
that button's ID, overlap flag, crop, gain and target for mouse and hotkey playback.
Stop ends every instance on both outputs. Snapshot instances identify all playing
pads independently of the selected pad, so simultaneous pads can stay illuminated. The worker's snapshots remain the authority on actual
playback state and the confirmed target.

Opening or closing Settings uses iced window tasks. `ThemeChanged`
replaces the optional override, which iced reads through the application theme
callback on the next update. Selecting a theme applies it immediately; Close
retains it, and Default theme sends `ThemeChanged(ThemeChoice::Deck)`. Theme changes queue a
settings save. `subscription` batches the existing audio stream with native
window-close events and an Escape-key listener carrying the originating window
ID. Escape closes Settings only from Settings, or dismisses the grid menu from
the main window. The audio subscription keeps the same identity, so opening
Settings does not restart the worker.

## View and layout

`view(&self, window)` borrows state for reading and returns `Element<'_, Message>`, iced's
common widget type. The `'_` lifetime lets Rust infer how long the interface may
borrow from state. Building a view cannot mutate the application.

The main column contains a growing grid area, an optional error message, and
the footer. It adds the error message only when needed, without leaving an empty
status row. The grid renders button records in saved order, inside a native
scrollable area. `button_dimensions` scales the original 180-pixel minimum width
and 86-pixel height by `sound_button_scale / 100`. The Settings slider changes
that saved percentage between 50 and 200 in steps of 5. The grid uses the scaled
width in `fluid` and the scaled height for each pad. Grid rows use natural sizing,
so additional rows grow the scrollable content rather than shrinking
all pads into one row's height. Padding allows the raised faces and shadows to
extend beyond cell bounds. Each label uses its saved custom name or filename
stem. The text box fills the pad's content area and centers its text on both
axes. `src/ui/fitted_label.rs` measures the actual monospace paragraph with iced's
text engine during layout. It starts at 18 pixels and scales down to the available
width. Names that would require less than 10 pixels wrap by glyph first, then
shrink further only when needed to fit the height. Measurements update with the
label and layout; short names recover their full size. A clipped container keeps
the content inside the cell. Sound pads do not
show hover tooltips; right-click still opens the pad menu.

`src/ui/sound_pad.rs` supplies the pad's layered surface and animation, following
the [pushable button design](https://www.joshwcomeau.com/animation/3d-button/) shown
in the reference screencast. It paints a separate ground shadow, a stationary
edge with darker corners, and a flat moving face. All colors come from the theme:
the face uses its primary pair, playback uses the primary strong pair with an accent backlight,
and disabled state uses the background pair. The edge is shaded from the face.

The wrapper accepts the existing iced `Button` and gives it a transparent surface
with the paired text color; the wrapper supplies the visible face behind it.
The wrapper forwards layout, input, cursor feedback, operations, and overlays;
the native button still decides whether a release should send `Play`.
The outer right-click `mouse_area` sends `OpenButtonMenu`.
When playback becomes unavailable, the wrapper recreates the child button's state
to cancel any held press; iced otherwise retains it while disabled. Ordinary view
rebuilding preserves the native button and animation state.

The wrapper stores an iced `Animation<bool>` in its persistent widget tree state.
It interpolates a `Layers` value containing the face offset, shadow offset, and
brightness. The face sits 4 pixels above the base at rest, 6 above on hover, and
2 above while pressed; the shadow moves down by 2, 4, and 1 pixels respectively.
Hover also brightens the face and edge by 10%.

Press feedback takes 34 ms, hover/release 250 ms with iced's `EaseOutBack`, and
mouse-out 600 ms with `EaseOutCubic`. Each phase change samples the current layer
values before starting its new timing, so changing direction mid-animation does
not jump. `draw` translates only the rendered face and native content; layout and
input bounds stay fixed, preventing repeated hover changes at the edge. It
requests another redraw only while interpolation is active, so no application-wide
animation subscription or audio change is needed.
The Cargo `advanced` feature exposes iced's widget interface without another
library. The native renderer can be replaced with iced's null renderer in tests
to exercise real pointer events without opening a window.

`row!` arranges footer controls horizontally; `column!` stacks the two volume rows.
Each volume row places its fixed-width percentage before the slider so the link
icon can sit 4 pixels beyond the tracks, centered between them.
`src/ui/volume_slider.rs` styles the native iced slider with an 8-pixel shaded
rail and an 18×22 rounded rectangular thumb. Its gradient face and dark lower
edge reuse the pad renderer's color shading helper. Hover brightens the face;
dragging reduces the visible lower edge. Native dragging, scrolling, and volume
messages continue to use the same slider widget. When the application is None,
Remote Volume uses a passive wrapper around that slider, with a muted palette and
no input processing. The wrapper preserves the thumb position and ignores mouse,
touch, wheel, and keyboard events. Switching between the wrapper and the native
slider resets its interaction state, so an interrupted drag cannot resume.
Local Volume stays interactive, and the existing link behavior still updates
both saved levels.
The `!` marks a macro, which generates code from its inputs. A horizontal space
with `Fill` pushes the application selector and Settings cog to the right. The main area
also uses `Fill` so the controls stay at the bottom as the window grows.

`src/ui/icons.rs` owns the icon font family, embedded bytes, and glyph mappings
for transport, recording, passthrough and other controls. Its text helper renders
Lucide glyphs without changing the application's monospace text font. Play and
Stop use 28-pixel icons on 64×64 surfaces; Settings uses a 28-pixel icon on a
64×64 surface. The 24×24 link control has a 16-pixel icon and a transparent style
in every state. Its glyph and accent text color signal linking.
`sound_pad::raised` shares the pad renderer with transport controls: Play uses the
theme's success palette and Stop a light-blue pair, retaining muted colors when
disabled. Record uses the danger palette and the same one-second blinking
widget as passthrough. Settings and Settings actions use the accent palette.
The main layout uses 12-pixel content/footer margins and footer gaps; the native application
selector is 220 pixels wide with 12-pixel text and 6-pixel padding. `TargetChoice`
omits descriptions after the first em dash and preserves the stream ID.
The font's pinned source, checksum, and license live in `assets/fonts/`.

Buttons without an `on_press` action are disabled. Footer controls do not show
hover tooltips. The link button's connected/broken-chain glyph and styling show
whether the volumes are linked. The application dropdown starts with None followed
by the discovered targets and has no heading or separate clear button.
`TargetChoice` supplies the display label for either an absent target or an
application. Both choices send `SelectTarget(Option<Target>)` through the worker's
existing `Command::Select` path, which stops playback when the selection changes.
Explicit None is sent even if no target is currently selected, so it can cancel
automatic selection before discovery finishes. The worker selects the first
available application by default; the UI displays its confirmed snapshot.
Routine PipeWire and playback status text is omitted; storage and audio errors
use the theme's danger text style independently.

Settings sends `OpenSettings`. `src/ui/windows.rs` opens a 440×420 native window
with a 360×260 minimum, or restores and focuses the existing Settings window.
The ID is recorded before scheduling creation, so repeated clicks cannot create
duplicates. `view` routes that ID to `src/ui/settings.rs` and the Options ID to
its editor; the main window uses the soundboard view. All share application state,
fonts, audio, and the selected custom theme.
The theme dropdown uses only `ThemeChoice::ALL` (Deck and Console). Settings has no backdrop, so the
main window remains usable. Close and Escape request native closure; its closed
event clears the ID so it can reopen. Stale close events cannot clear a newer
window. Closing the main window calls `iced::exit`, terminating both windows and
allowing the storage writer to flush. This explicit exit is necessary because
iced daemons otherwise continue running after their last window closes.

The Settings title appears only in the native title bar. The default window size
fits all controls without scrolling. The footer stays outside a scrollable
controls area, so Theme and Sound button size remain accessible at its minimum size.
`SoundButtonScaleChanged` clamps the percentage and queues a settings save only
when it changes. It updates layout without sending an audio command. All settings
saves include the current size so volume and theme changes cannot reset it.
The application list already updates through PipeWire registry listeners and
the worker's regular target refresh; no manual refresh control is needed.

`src/ui/tests.rs` tests state transitions without opening a window or file picker.
`src/ui/persistence/tests.rs` tests restoration, per-button assignment, autosave,
and stale save/picker results against real files inside `test-results/`.
`src/ui/sound_pad/tests.rs` checks native clicks, cancellation, assignment, and
animation settling with a null renderer. It also checks quick press feedback and
continuity when a hover or press transition is interrupted.
Visual layout and live interactions are checked with the
[manual acceptance checks](../acceptance.md).

## Application icon and desktop identity

`src/ui/windows.rs` supplies shared window settings for the main and Settings
windows. A `LazyLock` decodes the embedded `img/icon_full.png` once, scales it
to a 256×256 RGBA window icon, and clones it for each window. PNG decoding uses
`image` with only the PNG codec enabled. Runtime loading does not depend on the
working directory or a separate image file.

Both windows use the same Linux application ID as the audio graph:
`io.raspiboard.Raspiboard`. The matching launcher in
`assets/io.raspiboard.Raspiboard.desktop` supplies the desktop icon name and X11
window class. Wayland desktops resolve dock/application-menu icons from that
installed entry and image, as described in the README.


## Sound options and deletion

`button_options.rs` reuses the window settings and icon from `windows.rs`. One
options window targets a button ID and shows a color picker, editable name,
wrapping path, relative gain slider, crop editor with a waveform, and global
hotkey controls. Name edits queue the existing `Change::Button` save; they do not change the
active playback. Selecting another pad's Options reuses the window, and stale
name messages for its previous target are ignored.

`Change::DeleteButton` removes only the matching entry from its category through
an atomic rewrite, leaving source audio untouched. UI deletion updates the default
category count, stops an active pad, closes its options, and invalidates its
pending picker. Repeated deletion is safe, including an already empty category.


## Sound options editor

`options_state.rs` owns transient crop strings, validation errors, the decoded
waveform summary and its cancellation token. `crop_input.rs` reuses Iced AW's
NumberInput with a 0.01-second step and source-duration bounds. Decimal seconds
round to integer milliseconds before entering the existing string-based crop
validation/save path. The field's default source boundary normalizes to an
unspecified boundary. NumberInput's clear callback resets that boundary as soon
as its text is cleared. End zero remains an invalid numeric draft, so entering a
fractional End through a leading zero never resets to the file duration. Invalid
ranges never replace the saved crop.
`audio::PlaybackOptions::validate` is shared by storage, the editor and playback.
Generation, button ID and file path identify analysis results, preventing closed,
replaced or switched Options targets from receiving stale data. Analysis runs on
a dedicated worker, and closing/deleting/replacing cancels it.

The installed Iced AW color picker reuses Iced's widget/overlay handling. Its small
icon font is loaded by the application alongside Lucide. RGB colors are stored
canonically; the pad renderer uses the existing face and edge layers with native
readable text pairs and matching glow. Null color restores the current theme.

All three window views place their scrollable viewport outside the content
padding and use Iced's embedded scrollbar spacing. The bar reserves its width
at the window's right edge; inner containers retain the content margins. Footer
padding is separate, so controls remain fixed when the content scrolls. The main
grid's pointer coordinates now share the window origin and need no padding
offset when placing its context menu. Crop spinner buttons use transparent
backgrounds with theme text colors; the separator uses the bundled Lucide
right-arrow glyph through the existing icon helper.

`ui/waveform.rs` draws a bounded peak summary using Iced Canvas. A keyed column
resets widget state for each analysis generation. Drag previews update the crop
fields without writing to disk; release commits one valid range. Handle drags
clamp to source duration and preserve at least one millisecond. Clicking moves
the nearest edge. Full-source boundaries become null values when saved.
The handles use the theme's amber warning color, 1.5-pixel lines and small caps;
their original 10-pixel hit distance remains unchanged. `gain_slider.rs` wraps
the native slider in MouseArea so an ordinary hover wheel changes gain by 0.5 dB.
The child retains native Ctrl-wheel handling, and captured events cannot also
scroll the surrounding window or trigger a second step.

The Hotkey row below Crop opens the desktop chooser through `Action::SetHotkey`.
`src/ui/hotkeys.rs` builds a snapshot of enabled sound IDs and their descriptions;
Set includes the requested sound before saving any combination. A revision
identifies the snapshot. `Subscription::run_with` runs `src/hotkeys.rs`, which
owns a dedicated D-Bus connection, registers the application ID, creates a portal
session, and binds only those sounds. Dropping the subscription drops its
connection and sessions, including pending dialogs. No shortcut worker runs when
the snapshot is empty.

The portal chooses keys and reports human-readable trigger descriptions.
Assignments update the existing `Button::hotkey` field through `Change::Button`.
The snapshot stays stable when these labels change, so receiving a desktop update
does not restart the subscription. Activated signals use the existing
`play_button` path, preserving normal crop, gain, destination and audio checks.
Session IDs and action membership filter signals; a pressed set suppresses
repeats until Deactivated. Clear and deletion change the revision before the next
subscription update, invalidating queued assignments and activations immediately.
Desktop changes remain visible through ShortcutsChanged. Cancelled additions
restore a snapshot of the previously configured sounds. Portal errors appear in
Options and leave saved combinations and normal pad playback available.


## Application themes

`src/theme.rs` owns `ThemeChoice`, cached custom Iced palettes, and shared drawing
tokens. Native controls consume the explicit extended palette, while pads,
sliders and menus use the same surface/accent/border tokens for their custom
rendering. Deck uses near-black graphite surfaces; Console uses charcoal and
stronger metallic gradients. Idle default pads are neutral, while active pads use
an illuminated face and cyan outline/backlight. Per-pad RGB colors take priority.
Transport colors keep their meaning independently of the selected finish.

Settings selects only `ThemeChoice::ALL`. The renderer always receives an actual
application palette, so every native window, popup and waveform shares it. Theme
selection changes appearance and queues a settings save without audio commands.


## Mouse3 reordering

`src/ui/reorder.rs` wraps each pad with native `mouse_area` middle-press and
enter/exit messages. Stable IDs identify source and target; a stable outer `stack`
retains the mouse area's state while outline/insertion feedback changes. The
application event subscription observes middle release even when a child captured
the event. Escape, cursor exit, focus loss, picker opening and deletion cancel.

The main state keeps category IDs from the loaded entries so hovering cannot move
a pad to another category. Hovering changes only the target marker. On release,
the pad moves to the target position (after it when moving forward, before it
when moving backward), local sorting ranks are refreshed, and one reorder change
is queued. Playback/Options references remain stable by ID. Adding a sound still
appends it to the default group after any reordered default entries.


Options saves `Action::Overlap(bool)` through the existing `Change::Button` writer.
The checkbox changes the next playback request. Preview uses its own instance
identity, so its Stop control follows the actual local preview instead of whichever
pad was selected most recently. Deleting any pad still present in the worker's
instance snapshot stops the group, including an older overlapping pad.
