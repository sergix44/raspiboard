# Saved data reference

[Back to the README](../README.md) · [User guide](user-guide.md)

The first launch creates this layout in your home directory:

```text
~/.raspiboard/
├── settings.toml
└── categories/
    └── default.json
```

`settings.toml` stores `theme = "Deck"` or `theme = "Console"`, `remote_volume`,
`local_volume`, `volumes_linked`, and `sound_button_scale` (50–200, default 100).
`minimize_to_tray` is a boolean, defaulting to false when absent, that keeps the
app running in the desktop tray when closing the main window.
A missing theme setting uses Deck; Use default saves Deck explicitly. Built-in
Iced theme names and arbitrary theme names are rejected, with invalid settings
files preserved by the existing error handling.

Each category file contains its identity, display name, and sound entries:

```json
{
  "id": "01a19322-a325-71a8-b5ad-6d57adb395e2",
  "category_name": null,
  "entries": [
    {
      "id": "01a19322-a325-71a8-b5ad-76fcabdba7d8",
      "name": null,
      "full_path": "/home/you/Sounds/air-horn.wav",
      "start_ms": null,
      "end_ms": null,
      "color": null,
      "relative_volume": null,
      "overlap": false,
      "hotkey": null,
      "sorting": 1
    }
  ]
}
```

Category and entry IDs are UUIDv7 values. A null `category_name` identifies the
single default category, stored in `categories/default.json`. Named categories
use `categories/<category-id>.json`. The interface shows one grid: the default
category first, then named categories in filename order. Each category's entries
are ordered numerically by `sorting`; ties retain their JSON array order.
There is no separate layout file. New sounds go into the default category after
its existing entries, using the next sorting value.

Audio files stay in their original locations. `full_path` is an absolute path or
null for an unassigned button. A moved or missing audio file keeps its assignment and
reports an error when played. Null, empty, or whitespace-only names show the
sound's filename stem. The first launch includes one unassigned button; an
explicitly empty `entries` array stays empty.

`start_ms` and `end_ms` define the playback interval in milliseconds; null means
the beginning/end of the source. `relative_volume` is a gain in dB (null is 0 dB),
and `color` is an RGB hex value such as `#22CC88` (null uses the theme). Options
saves uppercase hex; valid existing lowercase hex is also accepted. `overlap`
controls whether a new instance is added to current playback; omitted values load
as false. Options saves the checkbox automatically. `hotkey` is the desktop-confirmed shortcut
description, or null when disabled. Choose combinations through Options or desktop
settings; editing this description does not change the desktop's registered keys.
Replacing a sound preserves its metadata and reloads its waveform. A crop that
exceeds the new duration must be corrected or cleared before playback.

Scrollbars stay at the right edge of the main, Settings and Sound options windows,
outside their content margins, with reserved width so they cannot cover controls.
Footers remain fixed while the content scrolls.

The grid scrolls when needed. Footer Play uses the most recently clicked or
assigned button; on startup it uses the first button. Add sounds through the
right-click menu; rename or delete them through each pad's menu. Hold the mouse
wheel button (Mouse3) on a pad, drag over another pad, and release to move it to
that position. An outline identifies the source and a cyan insertion edge marks
the destination. Order saves automatically on release and survives restart.
Releasing outside a pad, pressing Escape, or leaving/unfocusing the main window
cancels the drag. Reordering never starts or stops audio and works on unassigned
or offline pads too. Pads stay within their existing category groups; category
management controls are not implemented yet.

Saves run on a background worker in order, atomically replacing the affected
category file and finishing queued writes during normal shutdown. Invalid
configuration files remain intact. If loading fails, the app displays the file
error and uses temporary defaults with saving disabled for that launch; fix the
reported file and restart. Unknown fields are rejected so misspelled settings or
metadata are not silently discarded.

The previous individual-button files and `grid.json` are no longer read or
written. Existing files are left untouched; they are not automatically imported.
