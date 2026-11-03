<?php

declare(strict_types=1);

require_once __DIR__.'/common.php';
require_once __DIR__.'/graphics.php';

use function Raspiboard\Packaging\directory;
use function Raspiboard\Packaging\process;
use function Raspiboard\Packaging\packagePatterns;
use function Raspiboard\Packaging\verifyGraphicsBoundary;

function verifyChecksums(string $directory, string $format = 'all'): void
{
    $entries = file($directory.'/SHA256SUMS', FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES);
    $seen = [];
    foreach ($entries as $entry) {
        if (!preg_match('/^([a-f0-9]{64})  ([A-Za-z0-9_.+-]+)$/D', $entry, $match)) {
            throw new RuntimeException('Invalid checksum entry');
        }
        $file = $directory.'/'.$match[2];
        if (!is_file($file) || is_link($file) || isset($seen[$match[2]]) || !hash_equals($match[1], hash_file('sha256', $file))) {
            throw new RuntimeException('Checksum mismatch: '.$match[2]);
        }
        $seen[$match[2]] = true;
    }
    $packages = array_merge(glob($directory.'/*.deb'), glob($directory.'/*.AppImage'));
    $patterns = packagePatterns($format);
    if (count($seen) !== count($patterns) || count($packages) !== count($patterns)) {
        throw new RuntimeException('Unexpected package formats in SHA256SUMS');
    }
    foreach ($patterns as $pattern) {
        if (count(glob($directory.'/'.$pattern)) !== 1) {
            throw new RuntimeException('Expected exactly one '.$pattern);
        }
    }
    foreach ($packages as $file) {
        if (!isset($seen[basename($file)])) {
            throw new RuntimeException('Missing checksum: '.basename($file));
        }
    }
}

function verifyElfTree(string $root, bool $portable = false): void
{
    $files = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($root, FilesystemIterator::SKIP_DOTS));
    $count = 0;
    foreach ($files as $file) {
        if ($file->isLink() && (!str_starts_with((string) $file->getRealPath(), realpath($root).'/'))) {
            throw new RuntimeException('Symlink escapes package: '.$file->getPathname());
        }
        if (!$file->isFile()) {
            continue;
        }
        if (($file->getPerms() & 0022) !== 0) {
            throw new RuntimeException('Writable distribution file: '.$file->getPathname());
        }
        $path = $file->getPathname();
        if (file_get_contents($path, length: 4) !== "\x7fELF") {
            continue;
        }
        $count++;
        $header = process(['readelf', '--file-header', $path], capture: true);
        if (!str_contains($header, 'Advanced Micro Devices X86-64')) {
            throw new RuntimeException('Incorrect ELF architecture: '.$path);
        }
        $versions = process(['readelf', '--version-info', $path], capture: true);
        preg_match_all('/GLIBC_(\d+\.\d+(?:\.\d+)?)/', $versions, $matches);
        foreach ($matches[1] as $version) {
            if (version_compare($version, '2.41', '>')) {
                throw new RuntimeException('ELF needs glibc '.$version.': '.$path);
            }
        }
        $dynamic = process(['readelf', '--dynamic', $path], capture: true);
        foreach (explode("\n", $dynamic) as $line) {
            if (preg_match('/R(?:UN)?PATH/', $line) && preg_match('#/(?:source|output|cache)/#', $line)) {
                throw new RuntimeException('Build path in ELF loader configuration: '.$path);
            }
        }
        if (str_contains($dynamic, '(NEEDED)')) {
            $environment = $portable ? [...getenv(), 'LD_LIBRARY_PATH' => $root.'/usr/lib'] : null;
            $dependencies = process(['ldd', $path], environment: $environment, capture: true);
            if (str_contains($dependencies, 'not found')) {
                throw new RuntimeException('Unresolved dependencies: '.$path."\n".$dependencies);
            }
        }
    }
    if ($count === 0) {
        throw new RuntimeException('No ELF binaries found in '.$root);
    }
    echo "Verified $count x86-64 ELF files against glibc 2.41.\n";
}

function verifyPortable(string $root): void
{
    verifyGraphicsBoundary($root);
    foreach (['AppRun', 'usr/bin/raspiboard', 'usr/share/pipewire/client.conf',
        'usr/lib/libpipewire-0.3.so.0', 'usr/lib/spa-0.2/support/libspa-support.so',
        'usr/lib/spa-0.2/audioconvert/libspa-audioconvert.so',
        'usr/share/doc/raspiboard/LICENSE', 'usr/share/doc/raspiboard/THIRD-PARTY.txt',
        'usr/share/doc/raspiboard/SYSTEM-LIBRARIES.txt',
        'usr/share/applications/io.raspiboard.Raspiboard.desktop'] as $required) {
        if (!is_file($root.'/'.$required)) {
            throw new RuntimeException('Missing AppImage resource: '.$required);
        }
    }
    if (!is_executable($root.'/AppRun') || !is_executable($root.'/usr/bin/raspiboard')) {
        throw new RuntimeException('AppImage entrypoints are not executable');
    }
    verifyElfTree($root, true);
}

function verifyVersion(string $cargoVersion, ?string $debVersion, ?string $debName, ?string $appImageName): void
{
    // cargo-deb can use '~' to order a SemVer prerelease before the final release.
    $debianVersions = [$cargoVersion.'-1', preg_replace('/-/', '~', $cargoVersion, 1).'-1'];
    if (($debVersion !== null && !in_array($debVersion, $debianVersions, true))
        || ($debName !== null && basename($debName) !== 'raspiboard_'.$cargoVersion.'-1_amd64.deb')
        || ($appImageName !== null && basename($appImageName) !== 'Raspiboard-'.$cargoVersion.'-x86_64.AppImage')) {
        throw new RuntimeException('Version mismatch between Cargo, Debian metadata, and download names');
    }
}

function verifyArtifacts(string $directory, ?string $appdir = null, ?string $cargoVersion = null, string $format = 'all'): void
{
    verifyChecksums($directory, $format);
    $inspection = getcwd().'/target/package-inspection-'.bin2hex(random_bytes(4));
    directory($inspection);
    $deb = glob($directory.'/*.deb')[0] ?? null;
    $image = glob($directory.'/*.AppImage')[0] ?? null;
    if ($format !== 'appimage') {
        $fields = explode("\t", trim(process(['dpkg-deb', '--show', '--showformat', '${Package}\t${Version}\t${Architecture}\t${Maintainer}', $deb], capture: true)));
        if (count($fields) !== 4 || $fields[0] !== 'raspiboard' || $fields[2] !== 'amd64' || $fields[3] !== 'Sergio Brighenti <sergio@brighenti.me>') {
            throw new RuntimeException('Incorrect Debian package metadata');
        }
        if ($cargoVersion === null) {
            if (!preg_match('/^raspiboard_(.+)-1_amd64\.deb$/D', basename($deb), $match)) {
                throw new RuntimeException('Invalid Debian download name');
            }
            $cargoVersion = $match[1];
        }
        verifyVersion($cargoVersion, $fields[1], $deb, null);
        process(['dpkg-deb', '--extract', $deb, $inspection.'/deb']);
        foreach (['usr/bin/raspiboard', 'usr/share/applications/io.raspiboard.Raspiboard.desktop',
            'usr/share/icons/hicolor/256x256/apps/io.raspiboard.Raspiboard.png',
            'usr/share/doc/raspiboard/LICENSE', 'usr/share/doc/raspiboard/THIRD-PARTY.txt'] as $required) {
            if (!is_file($inspection.'/deb/'.$required)) {
                throw new RuntimeException('Missing Debian package resource: '.$required);
            }
        }
        verifyElfTree($inspection.'/deb');
        process(['desktop-file-validate', $inspection.'/deb/usr/share/applications/io.raspiboard.Raspiboard.desktop']);
    }
    if ($format !== 'deb') {
        if ($cargoVersion === null && preg_match('/^Raspiboard-(.+)-x86_64\.AppImage$/D', basename($image), $match)) {
            $cargoVersion = $match[1];
        }
        if ($cargoVersion === null) {
            throw new RuntimeException('Invalid AppImage download name');
        }
        verifyVersion($cargoVersion, null, null, $image);
        if ($appdir === null) {
            directory($inspection.'/appimage');
            process([glob($directory.'/*.AppImage')[0], '--appimage-extract'], directory: $inspection.'/appimage', capture: true);
            $appdir = $inspection.'/appimage/squashfs-root';
        }
        verifyPortable($appdir);
    }
    echo "Package checksums, metadata, resources, and libraries verified.\n";
}

if (realpath($_SERVER['SCRIPT_FILENAME']) === __FILE__) {
    try {
        if ($argc < 2 || $argc > 5) {
            throw new RuntimeException('Usage: php packaging/linux/verify.php ARTIFACT_DIRECTORY [APPDIR [CARGO_VERSION [FORMAT]]]');
        }
        verifyArtifacts(realpath($argv[1]) ?: $argv[1], ($argv[2] ?? '-') === '-' ? null : $argv[2], $argv[3] ?? null, $argv[4] ?? 'all');
    } catch (Throwable $error) {
        fwrite(STDERR, $error->getMessage()."\n");
        exit(1);
    }
}
