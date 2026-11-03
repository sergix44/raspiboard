<?php

declare(strict_types=1);

require __DIR__.'/common.php';
require __DIR__.'/notices.php';
require __DIR__.'/graphics.php';

use function Raspiboard\Packaging\copyTree;
use function Raspiboard\Packaging\directory;
use function Raspiboard\Packaging\process;
use function Raspiboard\Packaging\packagePatterns;
use const Raspiboard\Packaging\HOST_GRAPHICS_LIBRARIES;

$format = getenv('RASPIBOARD_PACKAGE_FORMAT') ?: 'all';
packagePatterns($format);
$work = '/output/work';
copyTree('/source', $work);
chdir($work);
putenv('RUSTFLAGS=--remap-path-prefix=/output/work=raspiboard --remap-path-prefix=/cache/cargo=rust-dependencies');
$target = 'x86_64-unknown-linux-gnu';
$metadata = json_decode(process(['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1'], capture: true), true, flags: JSON_THROW_ON_ERROR);
$package = $metadata['packages'][0];
$version = $package['version'];
file_put_contents('/output/build-info.json', json_encode(['version' => $version, 'format' => $format], JSON_THROW_ON_ERROR)."\n");
$assets = $work.'/target/package-assets';
$artifacts = '/output/artifacts';
$appdir = '/output/AppDir';
directory($assets.'/notices');
directory('/output/debug');
copy('/usr/bin/pw-config', '/output/debug/pw-config');
chmod('/output/debug/pw-config', 0755);
directory($appdir.'/usr/bin');
directory($appdir.'/usr/lib');

process(['php', 'tests/packaging/run.php']);
process(['cargo', 'fmt', '--check']);
process(['cargo', 'build', '--release', '--locked', '--target', $target]);
process(['cargo', 'test', '--release', '--locked', '--target', $target]);
process(['cargo', 'clippy', '--release', '--locked', '--target', $target, '--all-targets', '--', '-D', 'warnings']);

$compiled = getenv('CARGO_TARGET_DIR').'/'.$target.'/release/raspiboard';
$packageTarget = $work.'/target/package-binary';
$binary = $packageTarget.'/'.$target.'/release/raspiboard';
directory(dirname($binary));
copy($compiled, $binary);
process(['objcopy', '--only-keep-debug', $binary, '/output/debug/raspiboard.debug']);
process(['strip', '--strip-unneeded', $binary]);
process(['objcopy', '--add-gnu-debuglink=/output/debug/raspiboard.debug', $binary]);

$id = 'io.raspiboard.Raspiboard';
foreach ([16, 32, 48, 64, 128, 256, 512] as $size) {
    $directory = $assets.'/icons/'.$size.'x'.$size.'/apps';
    directory($directory);
    process(['magick', 'img/icon_full.png', '-resize', $size.'x'.$size, $directory.'/'.$id.'.png']);
}
writeNotices($assets.'/notices');
$changelog = "raspiboard ($version-1) unstable; urgency=medium\n\n  * Package Raspiboard for Debian and Ubuntu desktops.\n\n -- Sergio Brighenti <sergio@brighenti.me>  ".date(DATE_RFC2822)."\n";
file_put_contents($assets.'/notices/changelog.Debian.gz', gzencode($changelog, 9));
process(['desktop-file-validate', 'assets/'.$id.'.desktop']);
if ($format !== 'appimage') {
    process(['cargo', 'deb', '--no-build', '--no-strip', '--target', $target, '--output', $artifacts.'/raspiboard_'.$version.'-1_amd64.deb'], environment: [...getenv(), 'CARGO_TARGET_DIR' => $packageTarget]);
}

if ($format !== 'deb') {
    copyTree($assets.'/icons', $appdir.'/usr/share/icons/hicolor');
    copyTree($assets.'/notices', $appdir.'/usr/share/doc/raspiboard');
    copy('LICENSE', $appdir.'/usr/share/doc/raspiboard/LICENSE');
    copy('packaging/linux/appimage-runtime-LICENSE', $appdir.'/usr/share/doc/raspiboard/APPIMAGE-RUNTIME-LICENSE');
    directory($appdir.'/usr/share/applications');
    copy('assets/'.$id.'.desktop', $appdir.'/usr/share/applications/'.$id.'.desktop');
    copy($binary, $appdir.'/usr/bin/raspiboard');
    chmod($appdir.'/usr/bin/raspiboard', 0755);

    // client.conf names these modules; the server continues to run on the host.
    $modules = ['rt', 'protocol-native', 'client-node', 'client-device', 'adapter', 'metadata', 'session-manager'];
    directory($appdir.'/usr/lib/pipewire-0.3');
    foreach ($modules as $module) {
        $file = 'libpipewire-module-'.$module.'.so';
        copy('/usr/lib/x86_64-linux-gnu/pipewire-0.3/'.$file, $appdir.'/usr/lib/pipewire-0.3/'.$file);
    }
    foreach (['support', 'audioconvert', 'videoconvert'] as $plugin) {
        copyTree('/usr/lib/x86_64-linux-gnu/spa-0.2/'.$plugin, $appdir.'/usr/lib/spa-0.2/'.$plugin);
    }
    directory($appdir.'/usr/share/pipewire');
    copy('/usr/share/pipewire/client.conf', $appdir.'/usr/share/pipewire/client.conf');

    $deploy = ['linuxdeploy', '--appdir', $appdir, '--executable', $appdir.'/usr/bin/raspiboard',
        '--desktop-file', 'assets/'.$id.'.desktop', '--icon-file', $assets.'/icons/512x512/apps/'.$id.'.png',
        '--custom-apprun', 'packaging/linux/AppRun', '--deploy-deps-only', $appdir.'/usr/lib'];
    foreach (HOST_GRAPHICS_LIBRARIES as $pattern) {
        array_push($deploy, '--exclude-library', $pattern);
    }
    // Recording and keyboard libraries are opened dynamically; display clients come from the host.
    foreach (['libpipewire-0.3.so.0', 'libsndfile.so.1', 'libxkbcommon.so.0', 'libxkbcommon-x11.so.0'] as $library) {
        array_push($deploy, '--library', '/usr/lib/x86_64-linux-gnu/'.$library);
    }
    process($deploy, environment: [...getenv(), 'NO_STRIP' => '1']);
    chmod($appdir.'/AppRun', 0755);

    // Keep the distributor's copyright notices alongside bundled system libraries.
    $systemNotices = '';
    $systemPackages = [];
    $files = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($appdir.'/usr/lib', FilesystemIterator::SKIP_DOTS));
    foreach ($files as $file) {
        if (!$file->isFile()) {
            continue;
        }
        $name = $file->getFilename();
        try {
            $owner = process(['dpkg-query', '--search', '/usr/lib/x86_64-linux-gnu/*'.$name], capture: true);
            foreach (explode("\n", trim($owner)) as $line) {
                $systemPackages[explode(': ', $line, 2)[0]] = true;
            }
        } catch (RuntimeException) {
            throw new RuntimeException('Cannot identify the system library notice for '.$name);
        }
    }
    foreach (array_keys($systemPackages) as $systemPackage) {
        $name = explode(':', $systemPackage)[0];
        $path = '/usr/share/doc/'.$name.'/copyright';
        if (!is_file($path)) {
            throw new RuntimeException('Missing system copyright file: '.$path);
        }
        $systemNotices .= "\n===== $systemPackage =====\n".file_get_contents($path)."\n";
    }
    file_put_contents($appdir.'/usr/share/doc/raspiboard/SYSTEM-LIBRARIES.txt', $systemNotices);
    // Debian copyright files may reference these canonical texts rather than quote them.
    process(['cp', '-a', '/usr/share/common-licenses', $appdir.'/usr/share/']);

    process(['appimagetool', '--no-appstream', '--runtime-file', '/opt/downloads/runtime-x86_64',
        '--mksquashfs-opt', '-processors', '--mksquashfs-opt', getenv('CARGO_BUILD_JOBS') ?: '2',
        $appdir, $artifacts.'/Raspiboard-'.$version.'-x86_64.AppImage'], environment: [...getenv(), 'ARCH' => 'x86_64', 'VERSION' => $version]);
}

$checksums = '';
foreach (packagePatterns($format) as $pattern) {
    $file = glob($artifacts.'/'.$pattern)[0];
    $checksums .= hash_file('sha256', $file).'  '.basename($file)."\n";
}
file_put_contents($artifacts.'/SHA256SUMS', $checksums);
process(['php', 'packaging/linux/verify.php', $artifacts, $format === 'deb' ? '-' : $appdir, $version, $format]);
if ($format !== 'appimage') {
    process(['lintian', '--fail-on', 'error', $artifacts.'/raspiboard_'.$version.'-1_amd64.deb']);
}
echo "Packages built and structurally verified. Desktop checks remain manual.\n";
