<?php

declare(strict_types=1);

namespace Raspiboard\Packaging;

require_once __DIR__.'/common.php';

// Display clients must match the host's Mesa/proprietary driver libraries.
const HOST_GRAPHICS_LIBRARIES = [
    'libwayland-*.so*', 'libX*.so*', 'libxcb*.so*',
    'libEGL*.so*', 'libGL*.so*', 'libOpenGL*.so*',
    'libgbm.so*', 'libdrm*.so*', 'libvulkan*.so*',
];

function verifyGraphicsBoundary(string $root): void
{
    $files = new \RecursiveIteratorIterator(new \RecursiveDirectoryIterator($root.'/usr/lib', \FilesystemIterator::SKIP_DOTS));
    foreach ($files as $file) {
        foreach (HOST_GRAPHICS_LIBRARIES as $pattern) {
            if (fnmatch($pattern, $file->getFilename())) {
                throw new \RuntimeException('Host graphics library bundled: '.$file->getPathname());
            }
        }
    }
}

function verifyHostGraphics(string $root): void
{
    $drivers = array_merge(glob('/usr/lib/x86_64-linux-gnu/libEGL_mesa.so.*'), glob('/usr/lib/x86_64-linux-gnu/libvulkan_*.so'));
    $checked = [];
    foreach ($drivers as $driver) {
        $path = realpath($driver);
        if ($path === false || isset($checked[$path])) {
            continue;
        }
        $checked[$path] = true;
        $host = process(['ldd', '-r', $path], capture: true);
        $bundled = process(['ldd', '-r', $path], environment: [...getenv(), 'LD_LIBRARY_PATH' => $root.'/usr/lib'], capture: true);
        preg_match_all('/^(?:undefined symbol:|.*=> not found).*$/m', $host, $ordinaryErrors);
        preg_match_all('/^(?:undefined symbol:|.*=> not found).*$/m', $bundled, $bundleErrors);
        $introduced = array_diff($bundleErrors[0], $ordinaryErrors[0]);
        if ($introduced !== []) {
            throw new \RuntimeException('AppImage breaks host graphics driver '.$path.":\n".implode("\n", $introduced));
        }
    }
    if ($checked === []) {
        throw new \RuntimeException('No installed Mesa driver available for the host graphics compatibility check');
    }
    echo 'Verified host graphics compatibility for '.count($checked)." driver libraries without opening a display.\n";
}
