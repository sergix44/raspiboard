# Soundboard acceptance checks

Install the [build dependencies](development.md#build-and-run), then run from the
project directory in a graphical desktop session:

```sh
cargo run --locked
```

## Linux packages

Run these checks on Debian 13 and Ubuntu 26.04 x86-64 desktop sessions, with
PipeWire, a session manager, and the desktop's portal backend running. Exercise
Wayland and X11 where available. Build and check packages as described in
[Linux packaging](packaging.md).

1. Run `./scripts/package-linux --plain`. Expect a successful exit, one `.deb`,
   one AppImage, and `SHA256SUMS` under `target/packages/`. Expect no GUI to open.
   Run `(cd target/packages && sha256sum --check SHA256SUMS)`; both entries must
   report `OK`. A failed build must leave previous successful downloads intact.
   Repeat with `--format deb` and `--format appimage`; each successful run must
   expose only its selected package and `SHA256SUMS` under `target/packages/`.
2. Install with `sudo apt install ./target/packages/raspiboard_0.1.0-1_amd64.deb`.
   Launch Raspiboard from the menu. Expect the supplied icon, correct window
   grouping, and a title matching the version in `Cargo.toml`.
3. Choose an audio file through the desktop picker. Test local playback, then
   select a recording application and verify that it receives the sound alongside
   its microphone. Test changing volumes and stopping playback.
4. On a desktop with the Global Shortcuts portal, assign a hotkey, focus another
   application, and trigger the sound. Expect one activation per press. On a
   desktop without that portal, expect a clear error while pad playback remains
   available.
5. Save sound assignments and settings, close Raspiboard, and run
   `./target/packages/Raspiboard-0.1.0-x86_64.AppImage`. Expect the same saved data
   and working playback, routing, picker, and supported hotkeys. Repeat with
   extraction-based launch and, when used, the documented per-user menu entry.
   Hover and click several pads, resize the window, open Sound options, and switch
   themes. Expect intact pad faces, labels, and shadows, without rectangular
   missing regions or stale pixels. Compare with the installed `.deb` in the same
   desktop session if rendering differs.
   On KDE or another Wayland desktop supporting `xdg-toplevel-icon-v1`, launch
   the AppImage on an account without a Raspiboard desktop entry. Expect the
   embedded Raspiboard icon in the title bar and window switcher. Verify the main,
   Settings, and Sound options windows. On a desktop without that protocol,
   verify icon lookup after installing the matching menu entry.
6. With either format running, launch the other from a terminal. Expect
   `Raspiboard is already running.`, a successful exit, and one window. Close the
   app before switching formats for the remaining checks.
7. Run `sudo apt remove raspiboard`. Expect the installed executable/menu entry
   to be removed while `~/.raspiboard`, original sounds, and the AppImage remain.
   A separately installed per-user AppImage menu entry remains available.
8. Once two actual package versions exist, install the older `.deb`, save
   settings, then install the newer `.deb`. Expect the newer window version,
   preserved settings, and working playback. Reinstalling the same version is
   not an upgrade test.

## Application logs

1. Run `cargo run --locked` and inspect
   `~/.raspiboard/logs/raspiboard_rCURRENT.log`. Expect a `Starting Raspiboard`
   message with the version and lines formatted as
   `[YYYY-MM-DD HH:MM:SS.mmm] channel.LEVEL: message`.
2. Try playing an assigned file that has been moved away. Expect the playback
   error in both the interface and the log. Restore the file after the check.
3. Quit the app normally. Expect `Raspiboard stopped` in the log. Relaunch and
   expect the previous session in a timestamped archive and a new current log.
   Leave the app open across local midnight; its next log message must rotate
   the previous day's file.
4. With the app closed, create an expired diagnostic fixture:
   `touch -d '8 days ago' "$HOME/.raspiboard/logs/raspiboard_r2000-01-01_00-00-00.log"`.
   Relaunch. Expect that fixture to be removed and recent archives to remain.

## Single running instance

1. With Raspiboard open, run `cargo run --locked` in another terminal. Expect
   `Raspiboard is already running.`, a successful exit, and no additional window
   or interruption of the running app.
2. Repeat from another working directory using the absolute path to the same
   `target/debug/raspiboard` binary. Expect the same result.
3. Close the running app and run `cargo run --locked` again. Expect it to open
   normally even though `~/.raspiboard/instance.lock` still exists.
4. Terminate Raspiboard using the system's process manager, then launch it again.
   Expect normal startup without deleting the lock file.

## Layout and sound assignment

1. The main window's custom title bar shows the app icon and
   `Raspiboard v<version>` at the left, **Search…** centered in the window,
   and Minimize, Maximize/Restore and Close at the right, with no second system
   title bar. The version matches the package
   version in `Cargo.toml`. On first launch, expect one empty sound button at the
   top left of the grid area; later launches restore the saved grid. Play and Stop icons,
   Remote Volume and Local Volume with a chain toggle, an application dropdown,
   and a Settings cog sit along the bottom. There is no Target Application heading
   or routine PipeWire status line.
2. The empty button says Choose sound. Hovering shows no tooltip.
   Play is disabled until a sound and PipeWire connection are available. None
   still allows local playback and disables Remote Volume.
3. Right-click the sound button, choose Replace sound, and select a short mono or stereo PCM WAV. The
   button shows its filename without the extension; hovering shows no tooltip.
   Selecting a file does not play it.
4. Right-click again, choose Replace sound, then cancel. Expect the original assignment to remain.
   Repeated right-clicks must not open more than one file picker.
5. Resize the window down to its minimum size. The sound and all footer controls
   remain reachable. Short pad labels are centered horizontally and vertically;
   long filenames shrink to fit and very long names wrap without losing text.
   Try both a long custom name and filename, then resize: labels stay centered
   and regain their normal size when space allows. All interface text uses bundled
   Monaspace Krypton Regular, including the application dropdown, Settings,
   Sound options, crop inputs, and + Add Sound menu. No system installation of
   Monaspace is required.
   Remote Volume and Local Volume labels each stay on one line.
6. Hover the Play, Stop, chain, and cog buttons. No tooltips appear,
   including when controls are disabled. Expect crisp icons with no
   missing-character boxes; after switching between Deck and Console,
   icons follow the buttons' text colors.
7. Click the cog. Expect a separate Settings window with its own title bar;
   the soundboard remains undimmed and usable.
8. Expect 64×64 Play, Stop and Record buttons, colored green, blue and red respectively.
   Disabled controls retain muted versions of those colors. Settings is 64×64,
   and the link control is a 24×24 icon with no visible background or border,
   including on hover and press. Linked and unlinked glyphs remain distinct.
   Two short bracket lines above and below the chain indicate both slider rows;
   they use the accent color when linked and a muted text color when unlinked.
9. Play, Stop, Settings, Default theme, and Close share the pads' raised
   face, shaded edge, shadow, and press feedback. Click and drag outside each:
   no action fires. The dropdown remains a compact native selector.
10. Expect 12-pixel content/footer margins, tighter footer spacing, and a smaller
    application selector. Entries show application names and stream IDs, with no
    description or source text after an em dash. Different streams remain distinct.
    Fill the grid until it scrolls: the scrollbar sits against the window's right
    edge and takes its own width. It must not overlap the rightmost pads. Footer
    controls keep their margins and remain stationary. Right-click near both
    window edges: menus still open at the pointer and stay inside the window.
11. Both volume sliders have thicker shaded tracks and rounded rectangular thumbs
    with accent faces and darker lower edges. Hover brightens the thumb; dragging
    gives it a pressed appearance. Try Deck and Console: tracks and thumbs
    remain distinct. Percentages sit before the tracks, with the transparent
    link icon centered between the two rows, 4 pixels from their right edges.
    Check 0%, 50%, and 100%: thumbs stay within their tracks and dragging works
    across the full range. Linking still makes either slider change both values.

12. Confirm the supplied neon application icon identifies the main window and
    Settings in the title bar or app switcher where supported. On Wayland,
    first install the [desktop launcher](development.md#desktop-launcher-and-application-icon)
    and check the application menu and dock. Both windows should group under
    Raspiboard. Launch from another working directory: the icon still loads.

## Custom title bar and search

1. Run `cargo run --locked`. In both Deck and Console, check the icon, version,
   search glyph and three circular window controls: yellow Minimize, green
   Maximize/Restore, and red Close. Their bevel, shadow, hover and press motion
   match the existing raised buttons. The header blends into the main background,
   with a compact gap above the first pads. The window has the same corner radius
   and border as its panels, with transparent outside corners. Maximizing fills
   the screen with square outside corners. Settings and Sound options keep their
   opaque backgrounds and system decorations. Resize to the minimum width: search
   remains centered, the title stays readable and controls do not overlap.
2. Drag the title/icon or unused header space: the window moves. Double-click
   it to maximize and again to restore. Repeat with the Maximize/Restore button
   and desktop snapping: its icon follows the actual state. Drag each of the
   four edges and four corners to resize; check the matching resize cursor and
   minimum dimensions. Maximized windows do not expose active resize handles.
3. Click and select text in Search, including double-clicking a word. Click its
   border and clear button. None of those moves or maximizes the window. Click
   each window button, drag outside it and release: no action fires. Minimize
   hides the window; restore it through the desktop/task switcher.
4. Add pads named `Brass Horn`, `Air Horn`, and `Door Bell`. Search `  hOrN  `:
   only the two horns remain in their existing order. Search `bell`: only the
   bell remains. A custom label matches instead of the underlying filename.
   Search an absent name: expect **No sounds match your search**. Click **×**
   and expect the full board. Restart: search is empty.
5. Play a pad, then filter it out. Playback and bottom controls continue working;
   its global hotkey remains usable. Check recording and passthrough continue
   during filtering. Reorder visible matches, then clear search: all hidden pads
   remain, with their relative order preserved. Restart and confirm saved order.
6. Right-click pads and empty grid space with a query present. Menus appear at
   the pointer, including near the bottom/right edges. Changing the query closes
   an open menu or cancels a pending reorder. Settings and Sound options retain
   their system title bars.
7. Close the main window using the custom **×** with **Minimize to tray on close**
   disabled: the app exits. Enable the setting and repeat with a tray available:
   it keeps running in the tray. Choose **Show Raspiboard** and check search,
   window controls, and resize handles work in the restored window. Explicit
   tray Quit still exits and finishes any active recording.

## Application audio passthrough

1. Run `cargo run --locked` in a graphical desktop session. Play a browser video
   or an external player and open a recording application's microphone stream.
   Expect an upper source selector and lower destination selector beside a
   light-green Cable button, with a routing arrow on their left. Verify both
   themes and the minimum window size. The Cable icon must render without a
   missing-character box; the button and arrow are dim when off.
2. With No Input or No Output selected, expect the Cable button to be disabled.
   Select the playing source and receiving application, then click Cable.
   Expect a recording-light blink: one second on, one second off, with a brief warm-up
   and afterglow, plus a steady green arrow once routing connects. Verify the
   recording application receives only the chosen source,
   alongside the microphone. Another playing application's audio is not shared.
   The source must remain audible through its existing speakers or headphones.
3. Change Remote Volume to 0%, 50%, and 100%. Expect the receiving application's
   shared feed to change without changing your normal source listening level.
   With the volume chain unlinked, Local Volume must not change shared audio.
   Test both a mono source and a stereo source, including a mono destination.
4. Play a clip while sharing. Expect the source, clip and microphone together.
   Pause/resume and Stop the clip, and use Options Preview: sharing continues.
   Change the selected source: clips continue, but passthrough turns off until
   enabled again. Change the destination: both clips and passthrough stop.
5. Pause/resume the source while its stream remains open. Expect silence/resumed
   sharing without disabling passthrough. Close the selected source or recording
   stream. Expect passthrough to turn off, its arrow to dim, and an appropriate
   error. If a browser recreates a stream, select its new entry and enable again.
6. Toggle passthrough repeatedly, including while clips play. Expect no duplicate
   audio or lingering shared sound after disabling. Restart Raspiboard and expect
   sharing off. Disconnect/reconnect PipeWire: sharing must remain off after
   reconnection. Quitting Raspiboard restores ordinary source listening and
   microphone behavior without leaving any passthrough links behind.

## System recording and preferences

1. Run `cargo run --locked`. Open Settings and scroll to **Recordings**. Expect
   FLAC by default and `~/.raspiboard/recordings/` as the folder. Choose another
   folder and format, close/reopen Settings, then restart: preferences remain.
   Cancel folder picking and verify the old folder remains. Use default folder:
   the default is restored. Existing recordings must stay in their old folder.
2. Play a video or music, then click the red Record button. Expect Starting to
   become the same one-second on/off filament blink as passthrough, in red.
   Record again: expect Saving, then a saved path. Add that file to a pad and
   play it: audio matches the desktop output. Repeat with WAV and Ogg Vorbis;
   extensions match the selected format and all three files play in an external
   player too. No pad is added automatically and no existing file is overwritten.
3. Record a source, several seconds of silence, then more audio. Verify the
   silence remains in the file and duration matches the session. Speak into the
   default microphone without monitoring it to speakers: your voice must appear
   alongside desktop audio, without live microphone feedback. Test a mono mic
   and check its voice is present in both stereo channels. If the mic already
   feeds the desktop output, verify it is not doubled. Turn microphone monitoring
   on or off during capture: expect recording to stop and finalize with an error
   asking you to restart it, rather than silently losing or doubling voice. Audio
   sent to another output or only to a remote application must not appear.
4. Record while playing clips and sharing passthrough. Stop/Pause clips, use
   Options Preview, change either app selector and open the audio file picker.
   Recording continues, capturing the default desktop output and microphone.
   Stopping Record leaves clips and passthrough alone. Check both themes at the
   minimum window size: all toolbar controls and selectors remain reachable.
5. During recording, change format and folder. Finish: the current file uses its
   original preferences; the next file uses the new ones. Closing to the tray
   keeps recording. Quit from the tray or close normally: a playable file is
   saved before exit. Also click Record then Quit immediately; no hang or empty
   leftover file. Repeat quick Record/Record and record again after saving.
6. Choose an unwritable folder and try recording. Expect a visible error and no
   claim of a saved file. Disconnect the recording output or PipeWire during
   capture: expect recording to stop and captured data to be finalized where
   possible. After reconnection recording remains off. Verify a full/busy disk
   produces an error rather than a silently shortened successful recording.

## Sound Options name and color layout

1. Open a pad's Options. Name and its helper text align with the text field's
   left edge. The color swatch stays to its left, with a smaller Reset directly
   below the swatch. Verify at the minimum Options size and in both themes.
2. Click Prefill filename at the right of the Name row. Expect the filename
   without its extension to become editable text, replacing any custom name.
   Edit part of a long name, close/reopen Options, and verify it is saved.
   With no assigned file, Prefill filename is disabled. Prefill and color Reset
   must leave playback and the other pad settings unchanged.

## Long-file responsiveness

1. With recording off, trigger a long MP3/AAC pad with a crop well into the file.
   Expect prompt playback starting at the same saved crop, without scanning the
   entire selected range first. Repeat with WAV, FLAC and Ogg.
2. Open Options on a long sound. The first waveform load should finish faster;
   close and reopen Options on the same unchanged file and expect immediate reuse.
   Replace or externally edit the file, then reopen: expect a fresh waveform.
3. Start recording and repeat playback and Options checks. Recording must include
   desktop clips plus your default mic while the interface remains responsive.

## Adding sounds

1. Right-click empty grid space. Expect a menu near the pointer containing
   **+ Add Sound** in a compact, left-aligned row with 12-pixel text. The menu is
   152 pixels wide with a thin border, small corner radius, and subtle shadow.
   Hover highlights the row; it has no raised face or lift animation. Try near
   the window edges: the whole menu remains visible. Check Deck and Console.
2. Click outside or press Escape. The menu closes without activating a pad or
   changing a volume. Hovering controls still shows no tooltips.
3. Choose + Add Sound. Expect one audio file picker and the menu to close. Cancel:
   no button is added and the previous active sound remains selected.
4. Repeat and select a WAV. A new button appears after the existing pads with
   the filename stem. Existing assignments stay intact; no playback starts.
5. Add another sound, then restart the app. Both new pads and their order remain.
   Click each to play its own WAV; footer Play uses the most recently selected pad.
6. Add enough sounds to fill several rows. The grid scrolls and its empty areas
   still open the menu. Right-clicking an existing pad opens Options, Replace sound, and Delete.
   Only Replace sound opens the picker. At the minimum window size, the menu remains reachable.
7. While a picker is open, right-clicking the grid must not open another picker
   or menu. Settings can remain open while adding a sound from the main window.
   Adding a sound during playback stops both clip outputs.

## Sound button feel

1. Assign a WAV and select a connected recording application so playback is
   available. Expect a rounded accent-colored face above a solid darker edge,
   with a separate shadow underneath.
2. Move the pointer onto the button. Its face should lift and brighten while the
   ground shadow moves farther down. Hold the left mouse button: the face snaps
   down quickly, leaving a thin solid edge visible, and the shadow moves closer.
   Releasing starts playback immediately and springs the face up again. The base
   stays fixed. Its stronger accent signals loading or playback; motion must not
   delay audio.
3. Move the pointer slowly around the button's edges. The animation must not
   repeatedly enter and leave hover or shift the footer or neighboring cells.
4. Press, drag outside, and release. No new playback should start. The face
   settles slowly to its resting raised position. With Overlap Mode off, rapid ordinary clicks must
   still restart a single clip without overlapping copies, leaving the surface
   stuck down, or making the face/shadow jump when a transition reverses.
5. Choose None in the application dropdown. The assigned sound button stays
   interactive and plays through the desktop output only. Remote Volume is muted
   and ignores dragging, control-scroll, and arrow keys; Local Volume still works.
   Right-click and choose Replace sound to open the audio picker; canceling keeps the assigned file.
6. Try Deck and Console in Settings. The face, text, shaded edge,
   and playing highlight must follow each theme's accent palette and remain
   readable. Close Settings and check the animation again. No tooltip should
   appear over the sound button.
7. Leave the pointer still, both over and away from the button. The surface should
   settle rather than pulse continuously. With Settings open, the main window's
   pads still respond to clicks and animate normally.
8. If the PipeWire connection drops while the pad is held, that press must
   be canceled. Restoring playback availability must require a fresh click.

## Settings and themes

1. Open Settings. It starts at 580×600 in either theme. Scroll to the bottom:
   a Credits panel follows Recordings and shows the package version, Sergio
   Brighenti, GPL-3.0-only license, Rust/Iced/PipeWire, Monaspace Krypton and Lucide.
   The panel uses the theme's surface, border and corner radius. Default theme
   and Close remain visible below the scrollable content. Settings appears only
   in the native title bar; the content starts with Theme. Expand Theme.
   Only Deck and Console appear. Deck is the
   initial default: near-black panels, neutral graphite pads and cyan sliders.
   Choose Console: all windows change immediately to charcoal and stronger bevels,
   and Settings stays open. Built-in Iced themes are absent.
2. Move and resize Settings independently. Click the main window, play a pad,
   adjust volumes, change the application, and open the grid menu. Those controls
   work while Settings stays open. There is no dim backdrop.
3. Click the cog repeatedly. Only one Settings window exists. Minimize Settings,
   then click the cog: the same window is restored and focused. Click Close or
   the title-bar close button, then reopen it. The chosen theme remains.
4. Press Escape in Settings, including with the Theme dropdown open: Settings
   closes. Reopen Settings and press Escape in the main window: Settings stays
   open, and any grid menu closes. Clicking outside Settings leaves it open.
5. With Console selected, click Default theme. The selector changes to Deck and
   the button becomes disabled. The desktop preference and ICED_THEME environment
   variable do not replace the app's chosen theme.
6. Resize Settings to its minimum 360×260 size. Theme, Sound button size,
   and Credits remain reachable by scrolling; Default theme and Close stay visible.
   Open the dropdown and select Console without
   resizing. The main window keeps its own 1000×360 minimum size.
   Its scrollbar sits at the window's right edge, outside the content margins,
   and does not cover the dropdown arrow, text, or slider.
7. During playback, open Settings and change themes. Both outputs should continue
   without restarting or stopping. Close Settings and confirm the assigned sound,
   target, volume levels, and link state are retained. If the selected recording
   stream ends while Settings is open, the main window immediately shows the
   cleared application selection and error.
8. Relaunch Raspiboard. Expect the last selected theme and Settings closed. If
   Default theme was selected, the selector shows Deck.
9. Leave Settings open and close the main window, including during playback.
   Both windows close, the process exits, and clip playback stops. Restart to
   confirm pending theme and volume saves completed.
10. Move Sound button size through 50%, 100%, 150%, and 200%. Pad width and height
    change immediately, the grid reflows and scrolls as needed, and labels stay
    centered and fit their pads. Transport and other footer controls keep their
    sizes. At 100%, pads have their original size. Resize the main window at each
    setting: every pad remains reachable.
11. Change pad size during playback. Both outputs continue, the assigned sound
    and target stay selected, and volumes stay unchanged. Choose 150%, change the
    theme and volumes, close the app, and relaunch: size remains 150%.
12. Launch with an existing configuration that has no `sound_button_scale`:
    pads start at 100%. Default theme continues to reset only the theme.

## Saved settings and button grid

1. After launching with a fresh configuration, inspect `~/.raspiboard/`.
   Expect `settings.toml` and `categories/default.json`. The category has a UUIDv7
   ID, null `category_name`, and one entry with its own UUIDv7 ID and `sorting: 1`.
   Assigning a WAV writes its original absolute path to `full_path`; no WAV copy,
   individual button file, or `grid.json` appears.
2. With volumes unlinked, set Remote to 25%, Local to 70%, and choose Console. Assign
   a sound, close the app immediately, and relaunch. Expect the assignment, Console,
   and 25%/70% unlinked. Nothing plays automatically; the first available recording
   application is selected once discovery finds one.
3. Link the volumes, close, and relaunch. Expect both at 25% and linked. Choose
   Default theme, relaunch again, and expect the default theme selection.
4. Use + Add Sound from the grid's right-click menu to add a second sound.
   Restart: their order and names follow the saved files. Right-click and assign
   each button a different WAV.
   Expect both entries in `categories/default.json`, with increasing `sorting`
   values and all metadata keys present. Restart again; each must retain its own
   sound. Canceling a picker must not replace either assignment.
5. Select a recording application and click each configured sound. It must play
   the matching WAV. The footer control pauses the playing clip and resumes its held position.
   Stop ends playback; the next Play starts the selected clip from the beginning.
6. If the saved grid spans several rows, resize to the minimum window and scroll.
   Every row must remain reachable, with consistent button height and a stationary
   footer. Hover lifts and shadows must remain visible at the edges of the grid.

7. With the app closed, set the entries' `sorting` values to reverse their order,
   and set one name to an empty string. Relaunch: the order follows `sorting`
   and the empty name shows the audio filename stem. Reassign that WAV: its ID,
   sorting, and other metadata must survive.
8. With the app closed, add a named category JSON at
   `categories/<category-uuidv7>.json`, with unique UUIDv7 entry IDs. Relaunch:
   default entries appear first, followed by named entries. Add Sound inserts a
   new default entry before the named entries; restarting preserves that order.
   Reassigning a named entry updates its own category file only.
9. With the app closed, add trim, color, relative volume, overlap, and hotkey
   metadata to an entry. Relaunch and reassign its WAV, then close: all metadata
   survives. Crop, color, relative volume and overlap affect the next playback or
   pad appearance. A configured hotkey is restored through
   the desktop; its stored text alone does not configure the desktop's keys.
10. With the app closed, introduce a duplicate entry ID or a non-UUIDv7 ID in a
    category. Relaunch: an error identifies the category, files remain intact,
    and UI changes do not autosave until the file is repaired and the app restarted.

## Playback and routing

1. Start a call or microphone recording in an application you normally use.
   Keep its existing input-device selection. Use headphones to distinguish direct
   clip injection from sound captured by the physical microphone.
2. Expect the first available recording application to be selected automatically.
   If none are available at launch, the dropdown shows None and selects the first
   application when one appears. No audio starts automatically. The dropdown has
   a None entry above the applications and no separate clear button. If several
   streams appear, their IDs distinguish the entries; choose the one used by the
   call or recording.
   Start another recording application, then end its stream: the dropdown updates
   automatically without reopening the app or clicking a refresh control.
3. With volumes unlinked, set Remote Volume to 25% and Local Volume to 0%.
4. Click the assigned sound button while speaking. Verify in the receiving
   application that both your voice and the clip arrive. No clip should be audible
   through the headphones. The sound button loses its playing highlight and Stop
   becomes disabled when the clip ends.
5. Set Local Volume to 25% and Remote Volume to 0%, then click Play. Hear the clip
   through the desktop's default output. The application receives your voice with
   no injected clip. Changing either slider during playback changes only that
   output's clip level; it never changes microphone volume.
6. Set Remote to 25% and Local to 60%, then click the broken-chain button. Both
   become 25%; the icon shows a connected chain in the theme's accent color.
   Move either slider; both percentages and output levels move together.
   Click the connected chain to unlink; neither value jumps, and subsequent
   slider edits are independent.
7. Click Stop during playback. Both clip outputs stop and your microphone keeps
   working. Click Play again to restart the assigned clip.
8. Select None, then click a sound. Hear it through the desktop output, with no
   clip injected into any recording application. Remote Volume is disabled and
   keeps its value. Adjust Local Volume through 0%, 25%, and 60%; local playback
   follows it. Play a clip longer than one second to check it does not stall, and
   check that its playing highlight and Stop state reset when it ends.
9. With None selected, test linked and unlinked local volume changes, Stop,
   restart, and cancellation of the file picker. Select a recording application
   again: Remote Volume becomes interactive and the next Play uses both outputs.
   Repeat in Deck and Console; the disabled slider has no hover or drag
   feedback and never changes its thumb position through direct interaction.

Pass means the two outputs obey their respective controls and both your voice
and the injected clip reach the chosen stream without changing its microphone
device. A clip-only result, missing channel, crash, playback that never starts,
or loss of microphone audio is a failure. The receiving application's mute,
push-to-talk, and noise-suppression settings still apply.

## Additional checks

| Action | Pass condition |
| --- | --- |
| Click a sound pad with Overlap Mode off during playback | All current instances stop with a short outgoing fade; the clicked clip starts on both outputs. |
| Click the footer Pause button, then Play | Both outputs pause and resume from the held position. |
| Choose None or change the application during playback | The current clip stops; the next Play uses only local output for None, or both outputs for a selected app. |
| Choose None, then start or close recording applications | None remains selected, Remote Volume stays disabled, and local playback remains available. |
| Choose None or start local playback before any recording application appears | Later applications appear in the list but are not selected automatically; local playback keeps working. |
| Open the pad menu or Options during playback | Playback continues. |
| Choose Replace sound during playback, then cancel | Both outputs stop; the previous file remains assigned. |
| Assign a different WAV | Its name appears without starting playback; the next Play uses it. |
| Start a second recording application | It appears in the list without changing the current choice and receives no injected clip unless selected. |
| End the selected recording stream | Both outputs stop, selection clears, and an explanation appears. |
| Reopen that application's recording stream | It reappears but must be selected again. |
| Choose an invalid, missing, empty, or unsupported WAV | Play reports an error and the interface remains responsive. |
| Lose an output during playback | Both outputs stop and an error appears. |
| Play without an available desktop output | Startup fails with an error within about ten seconds; no clip starts on the remote output. |
| Close Raspiboard during playback | Both clip outputs stop; your original microphone route continues. |
| Relaunch Raspiboard | Saved buttons, theme, volumes, and link state return; playback is stopped and the first available application is selected. |
| Reconnect after a PipeWire disconnect | The first available application is selected again; playback remains stopped. |

Test both mono and stereo WAV clips if available. Keep logs or captures inside
the project's ignored `test-results/` directory:

```sh
mkdir -p test-results
cargo run --locked > test-results/routing-manual.log 2>&1
```

For a failure, report the selected application, visible error, each volume and
link state, the WAV's channel count/sample rate if known, and what was heard
locally and received by the application. Copy the terminal error if the window
fails to open.


## Audio formats and clean transitions

Run `cargo run --locked`, then select None for local-only playback.

1. Assign `jihad.wav` (or another clip with an abrupt nonzero ending). Let it
   finish, stop midway, and switch to a different pad while it plays. Repeat at
   different points: expect no sharp click at these transitions and no stuck
   playing indicator. A very short fade is intentional.
2. Repeat with a recording application selected. Both outputs should stop cleanly,
   and the microphone should continue normally. Switch pads rapidly and verify
   that old sounds do not resume or leave routes behind.
3. Assign files from `tests/fixtures/audio/` (and longer personal clips) covering
   FLAC, MP3, AAC/M4A, ALAC, Ogg Vorbis/Opus, AIFF, CAF, and Matroska/WebM
   Vorbis/Opus. Also try `cipolle lolle.ogg`, then pause/resume, Stop, and switch pads.
   Expect correct pitch and duration. Each container still requires a supported
   audio codec. Multichannel audio and unsupported codecs should report an error.
4. Stop during loading; then play another clip. Expect no delayed playback from
   the canceled clip. Restart a clip repeatedly and verify Stop still works.

These checks require listening on the user's PipeWire setup; unit tests do not
establish that every audible artifact is absent on that setup.


## Pause and resume

Run `cargo run --locked` and select None for local playback.

Each playing sound pad has a steady backlight matching its custom color or theme
accent around its edges. Check Deck and Console, with the pointer over and away
from the pads. Each pad stays illuminated while any of its copies is playing;
pausing or stopping all its copies, or the last copy finishing, turns it off.
Footer controls do not glow.

1. Play a long clip. The green footer button changes to a yellow Pause button
   with a black icon in every theme. Click it: audio
   fades out briefly, and the button changes back to green Play. Wait several seconds,
   then click Play: expect continuation from the held position, not the beginning
   or a position advanced by the waiting time.
2. Repeat with a recording application selected: both outputs pause/resume, and
   the application's microphone remains unaffected.
3. Pause, then Stop. Play again: expect the clip to start from the beginning.
   Pause and click another sound pad with Overlap Mode off: only the newly selected clip should play.
4. Toggle pause/resume repeatedly and adjust volume while paused. Expect no clicks,
   delayed replay, or stuck controls. Repeat near the clip's end: either it pauses
   with audio remaining or finishes normally, without a drain-timeout error.
5. When all instances are loading, the footer Play/Pause control is disabled;
   Stop remains available. If other instances are playing, Pause remains available.


## Sound button menu and Options

Run `cargo run --locked`.

1. Right-click a pad: expect Options, Replace sound, Delete near the pointer,
   with cog, replace-arrow, and trash icons. The empty-grid Add Sound item has a plus icon.
   At window edges and while scrolled, all three items remain reachable. Clicking
   outside or pressing Escape dismisses the menu. Empty grid space still offers
   only + Add Sound. Opening a menu does not stop playing audio.
2. Choose Options: expect a 760×760 window containing the editor controls, with no
   repeated heading inside the window. It can resize down to 560×480.
   Edit the name: the pad label updates immediately and survives restart.
   Clear it: the filename label returns. Verify long paths wrap and are reachable
   by scrolling at the minimum window size. Unassigned pads show No sound assigned.
3. Open Options on another pad: the same window shows that pad's name and path.
   Rename it and confirm no other pad changes. Close with Close, Escape, or the
   native window button. Reopening should work while Settings is also open.
4. Choose Replace sound: confirm the path updates in Options and the name stays
   the same. Canceling retains the old file. Only one picker may be open.
5. Delete an inactive pad: other audio continues. Delete any playing, loading or
   paused pad, including an older overlapping pad: all playback stops, its options window closes, and the source audio remains
   on disk. Restart: the deleted pad stays absent, including when it was the last
   pad in a category. + Add Sound still works on an empty board.


## Sound color, relative gain, and crop

Run `cargo run --locked`, assign a known sound, and open Options.

1. Compare with the supplied sketch: swatch beside Name, clear name action, path,
   relative volume with −15/0/+15 dB labels, Overlap Mode checkbox and helper,
   divider, Start/End and clear range,
   highlighted waveform and Close footer. At 760×760 and 560×480 all controls
   remain reachable through scrolling; long paths wrap. No duplicate heading.
   The scrollbar stays against the window's right edge, outside the content
   margins. The gain labels, name clear action and crop controls remain fully
   visible beside it. The spinner arrow buttons have transparent backgrounds
   with readable glyphs in both themes. A full-size Lucide arrow separates the
   crop inputs. The Preview and Close footer stays fixed with its usual margins.
2. Pick a color and confirm: pad face, readable label, and playing glow follow it.
   Cancel another choice, then use Reset under the color picker. Confirm no playback restart.
   Test both Deck and Console. Clear Name: filename fallback returns.
3. Choose −6 dB then +6 dB and restart the pad. Both local and remote audio should
   change relative to their master sliders. Setting a master to zero stays silent.
   Hover the Relative Volume slider and scroll up/down without Ctrl: it changes
   once per wheel event by 0.5 dB and stops at −15/+15 dB. The window does not
   scroll while the slider consumes the wheel. Scrolling elsewhere scrolls the
   window normally. Holding Ctrl must not cause a double step.
   Changes while playing/paused apply only on the next Play. Restart the app and
   confirm gain/color/range persist; original audio bytes must remain unchanged.
4. Start and End are numeric inputs with arrow buttons. Set only Start, only End,
   then both, using seconds such as 1.250. Hover either input and scroll without
   Ctrl: it changes by 0.01 seconds and the waveform follows. Arrow buttons use
   the same step. Defaults show Start 0 and End at the known source duration;
   clear either field to reset just that boundary. Playback
   should stay between those boundaries. Pause/resume retains the cropped position;
   Stop and replay begin at the cropped start. Crop endings should fade smoothly.
5. Try typing/pasting letters, a negative time, NaN, or a time beyond the known
   duration: numeric controls reject them. Paste 1.23456: it rounds to 1.235.
   Set End before Start: expect an inline error and the retained saved range.
   Correct it, then use the range clear button: full-source playback returns.
6. Drag in either direction to select a waveform range; drag each edge, including
   outside its bounds, and click to move the nearer edge. Fields and highlight
   agree, handles cannot cross, and playing matches the saved selection. Handles
   are thin amber lines with small caps, distinct from the cyan waveform, and
   remain easy to grab on both themes. Clearing
   the range restores the full waveform. Moving without releasing is a preview.
7. Switch Options between pads, close during waveform loading, replace the source
   or delete its pad. Old results must not reappear. A replacement shorter than
   its saved crop shows an error until the range is cleared/corrected. Missing
   audio gives an inline error while name and color stay editable. Unassigned pads
   show an empty state. Add/delete and Settings continue to work normally.


## Options action controls and preview

Run `cargo run --locked` and open a pad's Options.

1. Open the color picker: Cancel and Apply use the same raised faces, hover/press
   motion and label styling as the application's controls. Change a color and
   Apply; then open again, change it and Cancel. Verify apply commits the current
   picker color and cancel retains the previous color. Reopen several times and
   check that clicks and animation still work.
2. Confirm the crop arrow and clear button align with the input boxes; Start/End
   labels appear in a separate row. The trailing name/color/apply explanation
   is removed. Test initial and minimum window sizes.
3. Select a recording application, then choose Play preview in Options: only the
   local output auditions the current range and gain. The recording selection
   stays unchanged. The button changes from green with a Play icon to red Stop
   while loading/playing; Stop cancels playback and natural completion restores
   green Play preview. Normal playback from the grid still reaches both outputs.
4. Invalid crop input disables Play preview. While a preview plays, type an invalid
   crop value: red Stop remains usable. Switch Options to another pad and start
   its preview; only that pad's audio plays. Master Local=0 remains silent.


## Global sound hotkeys

Run `cargo run --locked` from a graphical desktop with a Global Shortcuts portal
backend (for example KDE). These are live desktop checks; automated tests do not
configure the desktop or open its chooser.

1. With no hotkeys configured, launch the app. No shortcut permission dialog
   appears. Open a sound's Options and scroll below Crop: Hotkey shows No hotkey
   set and Set. Preview and Close remain in the footer.
2. Click Set. In the desktop chooser assign a combination such as Alt+H and
   approve it. Options displays the actual approved combination and Clear. The
   sound must not play while choosing the keys. Cancel setup on a second sound:
   no shortcut is saved for it and the first sound's shortcut remains usable.
3. Focus another application, then minimize Raspiboard. Press the shortcut:
   its sound plays through the current outputs with its saved crop and gain.
   Holding the combination must not continuously restart it. Release and press
   again: it restarts once. Confirm with both None and a recording target selected.
4. Click the displayed combination to reopen desktop configuration. Change it:
   Options updates and saves the new value. Try a conflicting shortcut: use the
   desktop's conflict controls to cancel or resolve it. No extra in-app shortcut
   handler should play the sound a second time while Raspiboard has focus.
5. Click Clear, including during playback. Existing playback continues; future
   presses no longer play that sound. Set returns. Close and relaunch: it remains
   disabled even if the desktop remembers its old combination. Set can configure
   it again. Delete a configured sound: its old shortcut must not play another pad.
6. Configure two sounds, restart Raspiboard, and minimize it. Both shortcuts work
   without reopening Options. Test an unassigned sound, a missing file, and an
   offline audio connection: the app stays responsive and follows normal pad
   playback eligibility and error handling.
7. On a desktop without the portal, click Set: Options explains the failure;
   normal pad playback still works. Existing saved assignments must not be erased.
   On a version 1 portal, use desktop settings to change a returning shortcut;
   Options explains if it cannot open that configuration itself.

## Custom Deck and Console appearance

Run `cargo run --locked` and switch between Deck and Console in Settings.

1. Deck: near-black background, dark footer panel, graphite idle pads, crisp
   borders and cyan slider fills. Console: charcoal surfaces, sharper corners,
   and stronger hardware-style highlights/bevels. Hover and press animations work
   as before. No new player/deck controls appear.
2. Play a default-colored pad: its face and outline illuminate in cyan with a
   soft glow. Pause and Stop turn the glow off. A custom-colored pad keeps its
   assigned face/glow. Labels remain readable on very dark and bright colors.
3. Main/preview Play remains green, Pause yellow and Stop red. Disabled controls
   remain visibly disabled. Link controls and menus use cyan accents; menus,
   dropdowns, text inputs and color picker remain dark with readable placeholders,
   selected text, numeric labels and focus borders in both themes.
4. Open Settings and Sound options together; switch themes while audio plays or
   is paused. Both windows update without changing crop, gain, selection, volumes,
   or playback position. Preview and color picker Cancel/Apply remain usable.
5. Restart with each custom theme saved and check it restores. A missing theme
   setting starts in Deck. An invalid theme name reports its file error without
   overwriting the invalid settings file.


## Hold Mouse3 to reorder pads

Run `cargo run --locked` with at least three pads.

1. Hold the mouse wheel button on the first pad, move over the third, then release.
   Expect a source outline while held, an insertion marker after the target, then
   the order second/third/first. Move the last pad back over the first: the marker
   appears before that target and the moved pad becomes first. Restart to confirm
   each saved order restores. Left-click play and right-click menus still work.
2. Repeat while audio is playing and paused. The same sound/position and selected
   application stay active; no extra sound starts. Pad names, colors, crop, gain,
   Options target and original audio files are unchanged. Unassigned/offline pads
   can be reordered without enabling playback.
3. Hover targets but release over empty space or the footer: no order change.
   Repeat and press Escape, leave the window or switch focus: no stuck drag,
   unexpected move or save. Releasing on the source itself is a no-op.
4. Test multiple grid rows, different pad sizes, and an already scrolled grid.
   The insertion marker follows the pad under the pointer, with no stale marker
   after leaving it. The footer is not a drop target. Switching size or opening
   a picker cancels a drag safely.
5. Cancel a drag, delete its source or target, then reorder another pad: the
   deleted pad cannot reappear. Add a sound after reordering: it appends in the default
   group, and the live order matches a restart. Named-category pads reorder only
   within their own group; cross-category drops do nothing.

## First optimized release build

Build with `cargo build --release --locked -j 2`, then run
`RUST_BACKTRACE=1 ./target/release/raspiboard` from the checkout. The release profile
uses level-3 optimization, Thin LTO and one code-generation unit, with line tables
retained for crash diagnosis.

Repeat the routing/local-preview, pause/stop/switching, waveform crop/gain,
color-picker actions, theme switching, and Mouse3 reorder checks above. Confirm
normal shutdown persists settings and order, and relaunch restores them. The
optimized test suite runs with `cargo test --release --locked -j 2`. Keep any
crash logs and listening notes under `test-results/release/`.


## Overlap Mode

Run `cargo run --locked` with two long sounds and one shorter sound assigned.

1. Open a long sound's Options. Overlap Mode appears between Relative Volume and
   Crop, with the helper “Allow this sound to overlap itself or other sounds being
   played.” A new pad starts unchecked. Enable it and relaunch: it stays checked.
   Disable it and relaunch again: it stays unchecked. Replacing the audio preserves
   this setting. Enabling Overlap Mode immediately adds a small dot inside the
   pad's bottom-left corner; disabling it removes the dot. Check Deck and Console,
   including light and dark custom pad colors: the dot matches the readable label
   color and follows the face when hovered or pressed. It remains visible while
   idle, playing, paused, or unavailable, and after relaunch with overlap enabled.
   Ordinary pads and footer controls have no dot.
2. Enable overlap and click the pad five times quickly. Hear five independently
   started copies locally; the pad remains illuminated until the last finishes.
   Repeat via its global hotkey, including when another app has focus.
3. Start an ordinary long pad, then an overlapping short pad. Hear both. When the
   short sound ends, the long sound continues and its pad remains illuminated;
   the footer still offers Pause. Repeat with copies of the same long sound
   started at different times: finishing one copy must not clear the others.
4. While several copies play, use Pause, wait, then Resume. Every copy continues
   from its held position. Local/Remote volume changes affect all copies. Start
   an overlapping pad while paused: the existing group resumes and the new copy
   starts from its own beginning. Clicking an ordinary pad instead replaces the
   whole group.
5. Click Stop during overlapping playback, while paused and immediately after a
   burst of clicks during loading. Every copy stops, all glows turn off, and no
   delayed sound begins afterward. Replay to confirm the worker still responds.
6. Select a recording application and repeat steps 2–5. Both local monitoring and
   the selected app receive all copies; the microphone keeps working. Set one
   master volume to zero: only that output is muted. Changing the selected app,
   choosing None, or opening a file picker stops the entire group.
7. With copies playing, use Play preview in Options. The group stops and one
   local preview plays, with nothing sent to the selected app. Starting an
   overlapping pad replaces the preview with normal routed playback. Opening
   Options on another pad must not label ordinary playback as a preview.
8. While several copies play, start a missing or invalid audio file on an
   overlapping pad. Expect an error for that instance while existing audio
   continues. Delete an older playing pad: all copies stop. Close the main window
   with copies running: the process exits and audio stops.

## Platform separation regression

Run `cargo run --locked` on Linux. Confirm application names and stream IDs still
appear as before; local-only playback, previews, overlapping clips, pause/resume,
stop and linked/unlinked volumes retain their behavior. Close and reopen a target
recording stream: the stale selection must not route to a reused node. Configure,
activate and cancel a global shortcut and verify retry behavior. This refactor
adds no Windows runtime support or settings migration.

## System tray and close behavior

Run `cargo run --locked` in your graphical desktop session. Test on Wayland and
X11 where available, with a desktop supporting StatusNotifierItem (for example,
KDE Plasma or GNOME with an AppIndicator extension).

1. Leave **Minimize to tray on close** off. Expect the Raspiboard tray icon and
   Show Raspiboard/Quit menu. Close the main window; expect the process to exit
   and the tray icon to disappear.
2. Relaunch, enable the toggle in Settings, assign a global shortcut, and start
   a long sound. Close the main window while Settings/Sound options are open.
   Expect all app windows to close while audio and routing continue. Trigger
   the shortcut; expect playback without reopening the board.
3. Click the tray icon or choose **Show Raspiboard**. Expect one main window with
   the same assignments, settings, volumes, routing target, and playback state.
   Repeat hide/show and choose Show while already visible; expect one window.
4. Choose **Quit** while the window is closed to the tray. Expect the process to
   exit and the icon to disappear. Relaunch; expect the toggle to remain enabled.
   Disable it, relaunch, and verify closing exits again.
5. With the board closed to the tray, disable the desktop tray host/extension.
   Expect the main window to reopen. On a session without tray support, enable
   the setting and close the main window; expect the process to exit normally.
6. Close only Settings or Sound options using their title-bar buttons. Expect
   the main window to remain open, regardless of the tray preference.
