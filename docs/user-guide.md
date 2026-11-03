# Using Raspiboard

[Back to the README](../README.md)

## Build your board

Right-click empty grid space and choose **Add Sound** to create a pad from an
audio file. To change a pad's file, right-click it and choose **Replace sound**.
Choosing a file does not start playback. Opening the file picker stops current
playback; cancelling keeps the existing assignment.

Right-click a pad and choose **Options** to edit it:

| Setting | What it does |
| --- | --- |
| Name | Adds a custom label. Clear it to use the filename again. |
| Color | Sets the pad color. Theme color restores its default appearance. |
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

Click the tray icon or choose **Show Raspiboard** from its menu to reopen the
board. Choose **Quit** to exit and finish pending saves, regardless of the toggle.
The tray icon is available whenever the desktop supports StatusNotifierItem, on
both Wayland and X11. GNOME needs a tray extension such as AppIndicator support.
If no tray is available at startup, closing the main window exits normally; restart
after enabling desktop tray support. If the tray host disappears while the board
is closed, Raspiboard reopens the window so you can still reach it.
