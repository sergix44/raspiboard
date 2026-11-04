# Linux packaging

Raspiboard packages support x86-64 CPUs on Debian 13 and Ubuntu 26.04, with
newer releases as compatibility targets. The build uses Debian 13's libraries
and generic x86-64 CPU settings. Both formats use the same release executable,
application ID (`io.raspiboard.Raspiboard`), and settings in `~/.raspiboard`.

## Build packages

Requirements: an x86-64 Linux machine, PHP CLI 8.2 or newer, a working Podman
installation, network access, and enough space for the Rust build cache.
The compiler, native development dependencies, and packaging tools are installed
in the build container. PHP is required for building packages, not running them.

```sh
./scripts/package-linux
```

## Automated builds on GitHub

The **ci** workflow runs on every push to `master`, including
merged pull requests. It can also be started manually from the repository's
[Actions tab](https://github.com/sergix44/raspiboard/actions/workflows/package-linux.yml)
using **Run workflow**.

The **build-test-package** job builds both formats from one release executable
using `--format all`. It runs the full release test suite once, including library,
binary, integration, and documentation tests, plus formatting, Clippy, and the PHP
packaging checks. Tests are headless; no GUI or audio server is started. Both
packages must pass structural checks and Debian 13 / Ubuntu 26.04 compatibility
checks before either is uploaded. The compiler and native dependencies come from
the pinned Debian 13 tooling container.

Docker Buildx restores and saves the tooling container's layers through the
GitHub Actions cache, then loads the image for the packaging command's `--image`
option. A separate Cargo cache retains source downloads and compiled outputs.
Its key includes runner architecture, tooling inputs, Cargo inputs, and the
commit; restore prefixes reuse compatible earlier builds when source or
dependencies change. Cargo still checks fingerprints before reusing outputs.
Release optimizations and the two-job compilation limit apply to every build.

Open a successful run to download the `.deb`, AppImage, and shared `SHA256SUMS`
file directly, without ZIP wrappers. Put all three in the same directory and run
`sha256sum --check SHA256SUMS`. Run `chmod +x` on the AppImage before launching it.

Packages are retained for 14 days. Build logs are uploaded separately for 7 days,
including failed builds. The first run after a cache reset rebuilds the tooling
and dependencies; later runs can reuse them. These workflow downloads are not
GitHub Releases.

## Local build options

The default is two compilation jobs. Docker and job-count overrides are available:

```sh
./scripts/package-linux --engine docker --jobs 4
./scripts/package-linux --format deb
./scripts/package-linux --format appimage
./scripts/package-linux --engine docker --image localhost/raspiboard-packaging:debian13
./scripts/package-linux --plain
./scripts/package-linux --json
```

The default `--format all` builds both formats from the same executable. Selecting
`deb` or `appimage` builds and validates only that format. `target/packages/`
then contains the selected package and a `SHA256SUMS` file for that build.

`--image NAME` uses an already-built tooling image and skips building
`packaging/linux/Containerfile`; the image must contain the tools and native
dependencies defined there. Formatting, tests, Clippy, package generation, and
both compatibility checks still run. Without this option, the command builds
the tooling image normally using the selected container engine's local cache.

If your `docker` command is Podman's compatibility wrapper, use the default
Podman engine so the command applies rootless user-ID mapping correctly.

Normal output reports progress and the build log path. `--plain` works without
terminal formatting. `--json` writes one result object with `status`, output
directory and files, or the failed stage and log path; subprocess output stays
in the log. A nonzero exit status means the build or a required check failed.

The command performs these steps:

1. Build a Debian 13 tooling container with pinned Rust and packaging tools, or
   use the supplied `--image`.
2. Snapshot the application sources, excluding host build outputs and credentials.
3. Run formatting, release compilation, the Rust tests, and Clippy.
4. Generate icons and notices, preserve debug symbols, and package the executable
   using cargo-deb and linuxdeploy/AppImage tooling.
5. Verify archive contents, ELF architecture/library requirements, desktop files,
   checksums, and Debian package linting.
6. Check independent AppImage dependency resolution and Debian package
   installation/removal in disposable Debian 13 and Ubuntu 26.04 containers.
7. Update `target/packages` to point to the successful build's downloads.

For the current `Cargo.toml` version, outputs are:

```text
target/packages/
├── raspiboard_0.3.0-1_amd64.deb
├── Raspiboard-0.3.0-x86_64.AppImage
└── SHA256SUMS
```

The version comes from `Cargo.toml`. Previous runs remain under
`target/packaging/runs/`; failed builds leave the successful downloads in place.
`target/packages` is a symlink, updated after all automated checks pass. Move an
existing ordinary directory at that path aside before using the command.

The Cargo download and compilation cache lives in `target/packaging/cache/` and
is separate from normal host builds. Each run retains `build.log`, source and
package staging, and `debug/raspiboard.debug` for the matching executable.
Do not use debug symbols from another build when investigating a crash.

`packaging/linux/Containerfile` pins the compiler/base image.
`packaging/linux/tools.json` records packaging-tool URLs and SHA-256 checksums.
Some upstream tools publish through a moving `continuous` release: an upstream
replacement causes checksum verification to fail until the release has been
reviewed and the pin updated. Debian dependency updates are not frozen to a
repository snapshot, so builds are not promised to be byte-for-byte identical.

Only one packaging run can use this checkout's compilation cache at a time.
A second run exits with an explanatory error instead of mixing executables
from different source snapshots.

Lintian's `initial-upload-closes-no-bugs` warning is expected for these standalone
downloads: the generated changelog does not claim a Debian archive submission or
reference a Debian packaging-request bug. Other packaging findings should be
reviewed before a release.

## Install the Debian package

From the checkout, after building:

```sh
cd target/packages
sha256sum --check SHA256SUMS
sudo apt install ./raspiboard_0.3.0-1_amd64.deb
```

APT installs declared library and portal dependencies. A graphical desktop with
PipeWire and a session manager such as WirePlumber must already be configured.
Use the portal backend supplied by your desktop. File-dialog support alone
does not provide global hotkeys.

Launch **Raspiboard** from the application menu. Install a newer package with
the same `apt install ./...deb` command to upgrade. To uninstall:

```sh
sudo apt remove raspiboard
```

Settings, sound assignments, and original audio files remain in your home
directory. The package installs its executable, menu entry, icons, manual page,
and license notices under `/usr`.

## Run the AppImage

After checking the download's checksum:

```sh
chmod +x Raspiboard-0.3.0-x86_64.AppImage
./Raspiboard-0.3.0-x86_64.AppImage
```

For a system without FUSE, extract and run it:

```sh
./Raspiboard-0.3.0-x86_64.AppImage --appimage-extract
./squashfs-root/AppRun
```

Keep the extracted directory together. The launcher resolves libraries,
PipeWire modules and SPA plugins relative to that directory. Normal host client
configuration and user drop-ins retain their standard lookup order. Bundled
client configuration is a fallback when no ordinary host `client.conf` is
available. Explicit `PIPEWIRE_MODULE_DIR`, `SPA_PLUGIN_DIR`, and configuration
environment overrides are preserved.

The bundle uses the host's graphics drivers, PipeWire daemon, D-Bus session,
and portal service. It includes client libraries and resources; it does not
configure desktop audio services.

Wayland, X11/XCB, EGL/OpenGL, Vulkan, DRM, and GBM libraries come from the host
desktop alongside its graphics drivers. Bundling older display-client libraries
can prevent newer Mesa drivers from loading correctly. Packaging rejects those
libraries in the AppImage and checks the installed Mesa driver symbols against
the bundle's library path without opening a display.

## Package sizes and debug information

The release build retains debug information for diagnosing crashes. Packaging
extracts that information to `debug/raspiboard.debug` in the matching build run
and strips a copy of the executable for distribution. The build-cache executable
remains intact. The `.deb` and AppImage then compress the distributed files.

For example, the initial 0.1.0 build is about 92.7 MiB before stripping,
20.6 MiB after stripping, with 71.9 MiB of debug information retained separately.
Its compressed `.deb` is about 7.2 MiB; the AppImage is larger because it also
includes bundled libraries. Exact sizes change with the build and dependencies.

## Optional AppImage menu entry

On Wayland desktops supporting `xdg-toplevel-icon-v1`, the application sends its
embedded window icon directly, so its title-bar and window-switcher icon do not
require an installed launcher. X11 also supports an embedded window icon.
On other Wayland desktops, the matching launcher and installed icon provide
desktop lookup. The menu entry is optional when direct window icons work.

Use a stable per-user executable path so replacing an AppImage does not require
editing its launcher. These commands use the AppImage's extracted assets and
`desktop-file-install` from `desktop-file-utils`:

```sh
install -Dm755 Raspiboard-0.3.0-x86_64.AppImage "$HOME/.local/bin/raspiboard.AppImage"
./Raspiboard-0.3.0-x86_64.AppImage --appimage-extract
install -Dm644 \
  squashfs-root/usr/share/icons/hicolor/512x512/apps/io.raspiboard.Raspiboard.png \
  "$HOME/.local/share/icons/hicolor/512x512/apps/io.raspiboard.Raspiboard.png"
desktop-file-install --dir="$HOME/.local/share/applications" \
  --set-key=Exec --set-value="\"$HOME/.local/bin/raspiboard.AppImage\"" \
  squashfs-root/usr/share/applications/io.raspiboard.Raspiboard.desktop
```

The per-user entry takes precedence over an installed system entry with the same
ID. Choose the intended entry when testing each format. Test shortcut setup and
window grouping through the installed launcher as described in the
[acceptance checks](acceptance.md#linux-packages).

The Wayland protocol support is backported into the pinned window library;
see [the vendored window-library patch notes](../vendor/winit/PATCHES.md).

## Support-script checks

All packaging support programs and their behavior tests use PHP without Composer
dependencies. `AppRun` uses POSIX shell so the installed application needs no PHP.

```sh
php tests/packaging/run.php
shellcheck packaging/linux/AppRun
```

The automated containers do not open the GUI or exercise real desktop audio.
Run the [Linux package acceptance checks](acceptance.md#linux-packages) in actual
desktop sessions on both supported baseline distributions before a release.
