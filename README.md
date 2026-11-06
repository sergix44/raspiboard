<div align="center">

<img src="img/icon_full.png" alt="Raspiboard raspberry made of neon speakers" width="160">

# Raspiboard

**Your sounds. One click away.**

A desktop soundboard for Linux. Play clips through your speakers,
send them into a call, or do both at once.

<p>
  <img src="https://img.shields.io/badge/Linux-x86--64-151b23?logo=linux&logoColor=white" alt="Linux x86-64">
  <img src="https://img.shields.io/badge/Audio-PipeWire-00c9a7" alt="Audio through PipeWire">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPLv3-e6007e" alt="Licensed under GPL version 3"></a>
</p>

[Get started](#get-started) · [How to use](#your-first-sound) · [User guide](https://sergix44.github.io/raspiboard/guide.html) · [Build from source](docs/development.md)

</div>

---

## Make some noise

- **Play clips into a call.** Mix sounds alongside your microphone, or play
  through your speakers only. Separate volume sliders control each output.
- **Share application audio.** Send a browser, game, or audio player's sound
  into the selected call with the green Cable button.
- **Record desktop audio and your microphone.** Save as FLAC, WAV, or Ogg Vorbis.
- **Make each pad yours.** Change names, colors, volume, and playback crops.
  Enable Overlap Mode to layer sounds, or assign global hotkeys.*
- **Keep your board handy.** Search pads, drag to reorder, choose Deck or Console,
  and keep audio running in the system tray. Your board saves automatically.

<sub>*Global hotkeys need a desktop portal backend that supports Global Shortcuts.</sub>

## Get started

**Packages:** x86-64 Linux, targeting Debian 13+ and Ubuntu 26.04+.
Your desktop needs **PipeWire 0.3.65+**, a session manager such as WirePlumber,
and a desktop portal backend for file dialogs.

Download a `.deb` or AppImage and `SHA256SUMS` from
[Releases](https://github.com/sergix44/raspiboard/releases).
For development builds, open a successful
[build workflow run](https://github.com/sergix44/raspiboard/actions/workflows/build.yml)
and download its package and checksum artifacts.

Put the package and `SHA256SUMS` in the same folder, then verify your download:

```sh
sha256sum --ignore-missing --check SHA256SUMS
```

Your downloaded package should report `OK`. The examples below use version
`0.3.0`; use the filename you downloaded.

### Install the `.deb`

```sh
sudo apt install ./raspiboard_0.3.0-1_amd64.deb
```

Open **Raspiboard** from your application menu.

### Run the AppImage

```sh
chmod +x Raspiboard-0.3.0-x86_64.AppImage
./Raspiboard-0.3.0-x86_64.AppImage
```

Both formats share your settings and sound assignments. For AppImage extraction,
menu integration, or removal, see the [packaging guide](docs/packaging.md).

## Your first sound

1. **Add a clip.** Right-click empty grid space and choose **Add Sound**, then
   select an audio file. Use **Replace sound** to change an existing pad's file.
2. **Choose an output.** Join a call or open the receiving app's microphone
   stream, then select it in the lower application dropdown. Choose **No Output**
   for local playback only; no virtual microphone selection is needed in the call.
3. **Play and adjust.** Click a pad to play. **Local Volume** controls what you
   hear; **Remote Volume** controls what the receiving app hears. The chain icon
   links them. Use the bottom controls to pause, resume, or stop clips.
4. **Customize.** Right-click a pad → **Options** for its name, color, volume,
   waveform crop, overlap setting, or hotkey.

Hold the **middle mouse button** and drag a pad onto another to reorder it.
Use **Search…** to find pads by name. Open the settings cog for themes, pad size,
recording preferences, and **Minimize to tray on close**. On desktops with tray
support, click the tray icon to hide or restore the board; its menu offers
**Show** and **Quit**.

For sharing application audio, recording, seeking, and troubleshooting, see the
[user guide](https://sergix44.github.io/raspiboard/guide.html).

## Your files and settings

Play **WAV, FLAC, MP3, AAC/M4A, Ogg Vorbis, Opus**, and more. Mono and stereo
audio up to 384 kHz is supported. Cropping and volume changes leave your original
files untouched.

Raspiboard remembers audio file paths, so keep clips in a permanent folder.
If you move one, use **Replace sound** to select its new location.

Your board and settings live in `~/.raspiboard/`; recordings default to
`~/.raspiboard/recordings/`. To back up, close Raspiboard and copy that directory
**along with your original audio files**. Uninstalling the app keeps this data.

## Learn more

| Guide | What's inside |
| --- | --- |
| [User guide](https://sergix44.github.io/raspiboard/guide.html) | Playback, audio sharing, recording, shortcuts, and troubleshooting |
| [Linux packaging](docs/packaging.md) | Installation, package builds, and the release workflow |
| [Development](docs/development.md) | Build from source, run checks, and explore the Rust/Iced code |
| [Saved data](docs/saved-data.md) | Settings and board file formats |
| [Acceptance checks](docs/acceptance.md) | Manual desktop and package checks |

## License

Copyright 2026 Sergio Brighenti. Licensed under [GNU GPL version 3](LICENSE)
(`GPL-3.0-only`). Bundled dependencies and fonts retain their own licenses.
