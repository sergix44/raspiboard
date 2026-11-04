# Using Raspiboard

[Back to the README](../README.md)

## Window controls and search

The main window has a title bar with the Raspiboard icon and version on the left,
sound search in the center, and circular Minimize, Maximize/Restore, and Close
buttons on the right. The header blends into the board background and the
window's border follows the selected theme's panel corners.
Drag the title or unused header space to move the window; double-click it to
maximize or restore. Drag an edge or corner to resize.

Type in **Search…** to filter pads by their displayed name. Matching ignores
letter case and spaces at the beginning or end of the query. Custom names take
precedence over filenames. Click the search field's **×** to show the whole board
again. If nothing matches, the grid shows **No sounds match your search**.

Filtering leaves playback, recording, routing, and global hotkeys running. Pads
keep their saved assignments and order; you can reorder visible matches while
hidden pads keep their relative order. Search starts empty when the app launches.
Settings and Sound options open in separate windows with system title bars.
Settings opens at 580×600. Scroll below Recordings to find **Credits**, including
the app version, creator, license, and toolkit/font acknowledgments. The Default
theme and Close buttons stay visible while scrolling.

## Build your board

Right-click empty grid space and choose **Add Sound** to create a pad from an
audio file. To change a pad's file, right-click it and choose **Replace sound**.
Choosing a file does not start playback. Opening the file picker stops current
playback; cancelling keeps the existing assignment.

Right-click a pad and choose **Options** to edit it:

| Setting | What it does |
| --- | --- |
| Name | Adds a custom label. Clear it to use the filename again; Prefill filename copies the filename without its extension into the field for editing. |
| Color | Sets the pad color. Reset below the picker restores its default appearance. |
| Relative Volume | Adjusts this sound from −15 to +15 dB, on top of the master volumes. |
| Crop | Selects the part of the source to play. |
| Overlap Mode | Adds a new playback instance on every trigger. |
| Hotkey | Assigns a global shortcut through your desktop. |

Name and color edits apply immediately. Volume, crop, and overlap edits apply
the next time the pad plays. **Delete** removes the pad and its assignment,
leaving the original audio file on disk. Deleting a playing, loading, or paused
pad stops playback.

Hold the middle mouse button on a pad, drag onto another pad, and release to
reorder it. Press Escape or release outside a pad to cancel. Pad order saves
automatically. Pads stay within their existing category groups; the interface
does not yet offer category management.

## Playback and routing

Click a pad to play it from the start. The bottom **Play** button plays the most
recently clicked or assigned sound, or the first pad after launch. During
playback it becomes **Pause**; press it again to resume. **Stop** stops every
current sound, including sounds still loading.

With **Overlap Mode off**, triggering a pad stops the current sounds and starts
that clip. Triggering it again restarts it. With **Overlap Mode on**, each
trigger adds another instance without stopping the others. A small dot marks
pads with overlap enabled. Pause and Stop apply to all current instances.

When a sound's playable range is longer than five seconds and Overlap Mode is
off, a slim yellow progress bar slides up from behind the bottom panel and fills
as the sound plays. It spans the panel's full width, with rounded top corners and
square bottom corners extending behind the panel's rounded top edge for a flush
stacked appearance. The panel keeps its normal rounded corners when idle.
It uses the buttons' beveled
finish and grows slightly taller
on hover. Click to jump, or drag along the bar and release to seek backward or
forward. The bar covers the saved crop, so seeking stays between Start and End.
Seeking while paused keeps the sound paused until Resume. It preserves the
current output routing and volume.
The bar slides back behind the panel when playback stops or finishes. It is
hidden for previews, overlap-enabled sounds, and while more than one sound is
playing. For files without duration metadata, it appears after background
analysis finishes.

The application dropdown discovers recording streams automatically. An
application generally appears after it opens a microphone stream, such as when
joining a call or starting a recording. Select it to mix clips alongside its
existing microphone input. No virtual microphone selection is needed in the
receiving application.

Choose **No Output** for local playback only. **Local Volume** controls your
speakers or headphones; **Remote Volume** controls the selected application's
feed. The chain icon links or unlinks the sliders. Linking copies Remote Volume
to Local Volume. A zero volume mutes that output.

Changing the selected application stops current playback. If its recording
stream closes, playback stops and the dropdown returns to No Output. Discovery
may select the first available recording stream after startup or reconnection,
but never starts playback by itself.

## Application audio passthrough

Use passthrough to share a browser video, game, or another audio player's sound
with the selected recording application:

1. Start audio in the source application so it opens a playback stream.
2. Choose it in the upper **No Input** dropdown, then choose the receiving
   application in the lower output dropdown. Stream IDs distinguish multiple
   streams from one application, including separate browser streams.
3. Click the green **Cable** button. Once connected, it blinks with a one-second
   on/off rhythm and a brief filament fade. The routing arrow on its left stays
   green. Click again to stop sharing.

**Remote Volume** controls both shared application audio and soundboard clips.
You still hear the source through its normal desktop output and volume controls;
Raspiboard's Local Volume controls soundboard clips only. Clips and the existing
microphone can play alongside passthrough. Clip Play, Pause, Stop, and Options
Preview leave passthrough running.

Passthrough starts off every launch. Changing either dropdown turns it off;
enable it again to share the new selection. Closing the selected source or
destination, or losing the audio connection, also turns it off. If a browser
recreates its playback stream between videos, select its new stream and enable
passthrough again. Pausing a source while its stream remains open keeps sharing
enabled. Mono and stereo sources are supported.

## Recording desktop audio

Click the red **Record** button beside the blue Stop button to record the default
speakers/headphones mix together with your default microphone. Once capture connects, Record blinks red with the same
one-second on/off rhythm and brief filament fade as passthrough. Click Record
again to finish and save; the saved file's path appears above the toolbar.
Starting and saving have their own status text. Wait for saving to finish before
starting another recording.

**Stop affects clip playback only.** Recording continues through clip Pause,
Stop, pad changes, file picking, Options Preview and passthrough changes. The
default microphone is mixed into the recording without being played through
your speakers. If it is already monitored into the desktop output, its existing
feed is used so it is not added twice. Both default devices must be available.
Remote-only sound and audio sent to other output devices are not included.
The output monitor follows PipeWire's existing volume/mute configuration;
Raspiboard does not change the system listening volume to make a recording.

In **Settings → Recordings**, choose **FLAC** (default), **WAV**, or **Ogg Vorbis**,
and use **Choose folder** to select the output folder. **Use default folder**
restores `~/.raspiboard/recordings/`. Preferences save automatically and apply to
the next recording; an active file keeps its original format and location.
Files have unique names and the chosen extension. Existing recordings are never
moved or overwritten, and no pad is added automatically.

Capture uses 48 kHz stereo. FLAC and WAV preserve 24-bit PCM; Ogg Vorbis is lossy.
The WAV size limit is about four hours at this format. Encoding or disk errors,
including a full capture queue, stop recording and show an error rather than
silently dropping audio. A partial file is finalized where possible, and the
error includes its path. A recording stopped before receiving audio leaves no
empty file.

Closing to the tray keeps recording. A normal Quit waits for the recording to
finish saving; if that fails, the window remains available with an error. Losing
the audio output, microphone, or PipeWire connection stops recording; changing
the default output or input, or changing microphone monitoring, also stops
recording with a visible message. Reconnection does not
resume it. Recording always starts off after launch.

The Settings cog and all three transport buttons are 64×64. Settings remains
available while recording.

## Crop and preview

Open a pad's **Options** to view its waveform. Drag across it or adjust the
Start and End fields to select a playback range. Click the waveform to move
the nearest edge. Clear a boundary to use the beginning or end of the source;
the clear button resets the whole crop.

Times are in seconds. A crop whose End is before Start is rejected. Replacing
the file keeps the pad's options, so check its crop if the new file is shorter.

The green **Preview** button auditions locally using Local Volume; it
does not send audio to the selected application. Preview replaces current
playback and becomes a red **Stop** button while active. Original audio files
are never modified.

A red vertical line follows the preview's current position on the waveform,
starting at the crop's Start time. The amber crop handles stay in place. Pausing
holds the line still; stopping or finishing the preview removes it.

## Global hotkeys

In **Options → Hotkey**, click **Set** and choose a combination in the desktop's
shortcut chooser. Click an assigned combination to change it, or **Clear** to
disable it. Shortcuts use the pad's crop, volume, and overlap setting, and work
while Raspiboard is minimized or another application has focus.

Your desktop manages shortcut permissions, conflicts, and key combinations.
Holding a shortcut does not repeatedly restart the sound; release it before
pressing again. Clearing a shortcut in Raspiboard disables it, although the
desktop may remember the old combination.

A desktop backend implementing the **Global Shortcuts portal** is required.
File-dialog support alone is not enough. If the portal is unavailable, Options
shows an error and normal pad playback stays available. Editing an existing
shortcut through the chooser requires version 2 of that portal; older desktops
can change it in desktop settings.

## Appearance

Open the settings cog to choose **Deck**, the default dark theme with cyan
illumination, or **Console**, with stronger metallic edges. Change **Sound
button size** from 50% to 200% to fit more pads or make them easier to reach.
The grid reflows immediately, and appearance changes keep audio playing.

## Common questions

### Why isn't my recording application listed?

Open its microphone or recording stream first. Applications can expose more
than one stream; the IDs in the dropdown distinguish them. The list updates
automatically. Its mute, push-to-talk, and noise-suppression settings still
affect what other people hear.

### Why can't I hear a sound?

Check that PipeWire is running and your desktop has a default audio output.
Check Local Volume for what you hear and Remote Volume for the receiving
application. Confirm the pad's file still exists and its crop is valid.
For a local-only check, choose No Output and play the pad.

### Why doesn't a moved file play?

Raspiboard remembers the file's absolute path. Right-click the pad, choose
Replace sound, and select its new location. Keep audio files in a permanent
folder when possible.

### Why is the window icon generic?

X11 and Wayland desktops supporting `xdg-toplevel-icon-v1` can use the embedded
window icon directly. Other Wayland desktops need the matching installed
desktop entry. See [AppImage menu integration](packaging.md#optional-appimage-menu-entry).

### Where is my board saved?

In `~/.raspiboard/`. Close the application before backing it up, and include the
original audio files separately. The [saved data reference](saved-data.md)
documents the file layout. Invalid settings are preserved; if loading fails,
the app displays an error and disables saving for that launch. Fix the reported
file and restart.

### Why does launching it again do nothing?

One instance runs per user data directory. A second launch exits with
`Raspiboard is already running.` without interrupting the existing instance.
Do not delete `~/.raspiboard/instance.lock` while the app is running. A leftover
lock file after closing the app is normal.

## System tray

Enable **Minimize to tray on close** in Settings to keep Raspiboard running when
you close the main window. The setting saves immediately and defaults to off.
Audio playback, routing, and global shortcuts continue while the window is closed.
Settings and Sound options close with the main window.

Click the tray icon to toggle the board: it reopens a hidden or minimized window,
and hides an open window in the tray. This works regardless of the close setting.
Audio playback, recording, routing, and shortcuts continue while hidden.
Choose **Show Raspiboard** from the tray menu to always reopen or focus the board.
Choose **Quit** to exit and finish pending saves, regardless of the close setting.
The tray icon is available whenever the desktop supports StatusNotifierItem, on
both Wayland and X11. GNOME needs a tray extension such as AppIndicator support.
If no tray is available at startup, closing the main window exits normally; restart
after enabling desktop tray support. If the tray host disappears while the board
is closed, Raspiboard reopens the window so you can still reach it.
