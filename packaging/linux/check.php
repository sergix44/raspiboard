<?php

declare(strict_types=1);

require __DIR__.'/verify.php';

use function Raspiboard\Packaging\directory;
use function Raspiboard\Packaging\process;
use function Raspiboard\Packaging\verifyHostGraphics;

$artifacts = $argv[1] ?? '/output/artifacts';
$build = json_decode(file_get_contents('/output/build-info.json'), true, flags: JSON_THROW_ON_ERROR);
directory('/validation');
chdir('/validation');
file_put_contents('/usr/sbin/policy-rc.d', "#!/bin/sh\nexit 101\n");
chmod('/usr/sbin/policy-rc.d', 0755);
process(['apt-get', 'install', '-y', '--no-install-recommends',
    'libegl1', 'libgl1', 'libvulkan1', 'libegl-mesa0',
    'libwayland-client0', 'libwayland-cursor0', 'libwayland-egl1',
    'libx11-6', 'libx11-xcb1', 'libxcb1', 'libxcb-xkb1',
    'libxcursor1', 'libxi6', 'libxrandr2', 'libxrender1', 'libxfixes3', 'libxinerama1', 'desktop-file-utils']);
verifyChecksums($artifacts);
directory('/validation/portable');
process([glob($artifacts.'/*.AppImage')[0], '--appimage-extract'], directory: '/validation/portable', capture: true);
// Inspect the bundle before the .deb can provide otherwise missing libraries.
verifyPortable('/validation/portable/squashfs-root');
verifyHostGraphics('/validation/portable/squashfs-root');

if (!is_readable('/etc/pipewire/client.conf') && !is_readable('/usr/share/pipewire/client.conf')) {
    $probe = '/validation/configuration-probe';
    directory($probe.'/usr/bin');
    directory('/validation/empty-home');
    copy('/validation/portable/squashfs-root/AppRun', $probe.'/AppRun');
    copy('/output/debug/pw-config', $probe.'/usr/bin/raspiboard');
    chmod($probe.'/usr/bin/raspiboard', 0755);
    symlink('/validation/portable/squashfs-root/usr/lib', $probe.'/usr/lib');
    symlink('/validation/portable/squashfs-root/usr/share', $probe.'/usr/share');
    $environment = array_filter(getenv(), static fn ($key) => !preg_match('/^(PIPEWIRE_|SPA_|APPDIR|LD_LIBRARY_PATH)/', $key), ARRAY_FILTER_USE_KEY);
    $configuration = process(['sh', $probe.'/AppRun', '-n', 'client.conf', 'paths'], environment: [...$environment, 'HOME' => '/validation/empty-home', 'XDG_CONFIG_HOME' => '/validation/empty-home/.config'], capture: true);
    if (!str_contains($configuration, $probe.'/usr/share/pipewire/client.conf')) {
        throw new RuntimeException('Portable launcher did not load its fallback configuration: '.$configuration);
    }
    echo "Native PipeWire inspection verified bundled configuration fallback.\n";
}

directory('/root/.raspiboard');
file_put_contents('/root/.raspiboard/settings.toml', "packaging-removal-sentinel\n");
file_put_contents('/root/original-sound.wav', 'original-sound-sentinel');
process(['apt-get', 'install', '-y', '--no-install-recommends', glob($artifacts.'/*.deb')[0]]);
verifyArtifacts($artifacts, '/validation/portable/squashfs-root', $build['version']);
if (!is_executable('/usr/bin/raspiboard')) {
    throw new RuntimeException('Installed executable is missing');
}
process(['dpkg', '--remove', 'raspiboard']);
if (file_exists('/usr/bin/raspiboard') || file_get_contents('/root/.raspiboard/settings.toml') !== "packaging-removal-sentinel\n" || file_get_contents('/root/original-sound.wav') !== 'original-sound-sentinel') {
    throw new RuntimeException('Package removal failed to preserve user data');
}
echo "Clean baseline: install, removal, and independent portable dependency checks passed.\n";
