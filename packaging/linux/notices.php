<?php

declare(strict_types=1);

require_once __DIR__.'/common.php';

use function Raspiboard\Packaging\directory;
use function Raspiboard\Packaging\process;

function writeNotices(string $destination): void
{
    directory($destination);
    $metadata = json_decode(process(['cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-gnu'], capture: true), true, flags: JSON_THROW_ON_ERROR);
    $resolved = array_column($metadata['resolve']['nodes'], null, 'id');
    $text = "Third-party Rust dependency notices\n\n";
    $inventory = [];
    foreach ($metadata['packages'] as $package) {
        if ($package['name'] === 'raspiboard' || !isset($resolved[$package['id']])) {
            continue;
        }
        $base = dirname($package['manifest_path']);
        $text .= "\n===== {$package['name']} {$package['version']} =====\n";
        $text .= 'License: '.($package['license'] ?? 'see included license file')."\n";
        $text .= 'Repository: '.($package['repository'] ?? '')."\n";
        $licenses = [];
        $files = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($base, FilesystemIterator::SKIP_DOTS));
        foreach ($files as $file) {
            if ($file->isFile() && preg_match('/^(licen[cs]e|copying|copyright|notice)([._-].*)?$/i', $file->getFilename())) {
                $licenses[] = $file->getPathname();
            }
        }
        if ($package['license_file'] !== null && is_file($base.'/'.$package['license_file'])) {
            $licenses[] = $base.'/'.$package['license_file'];
        }
        sort($licenses);
        foreach (array_unique($licenses) as $license) {
            $text .= "\n--- ".substr($license, strlen($base) + 1)." ---\n".file_get_contents($license)."\n";
        }
        $inventory[] = ['name' => $package['name'], 'version' => $package['version'], 'license' => $package['license'], 'repository' => $package['repository'], 'source' => $package['source']];
    }
    foreach (glob('assets/fonts/*LICENSE') as $file) {
        $text .= "\n===== ".basename($file)." =====\n".file_get_contents($file)."\n";
    }
    file_put_contents($destination.'/THIRD-PARTY.txt', $text);
    file_put_contents($destination.'/THIRD-PARTY.json', json_encode($inventory, JSON_THROW_ON_ERROR | JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES)."\n");
}
