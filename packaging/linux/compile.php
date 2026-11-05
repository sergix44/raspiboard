<?php

declare(strict_types=1);

use function Raspiboard\Packaging\directory;
use function Raspiboard\Packaging\process;

process(['cargo', 'fmt', '--check']);
process(['cargo', 'clippy', '--release', '--locked', '--target', $target, '--all-targets', '--', '-D', 'warnings']);
process(['cargo', 'build', '--release', '--locked', '--target', $target]);
process(['cargo', 'test', '--no-run', '--release', '--locked', '--target', $target]);

$compiled = getenv('CARGO_TARGET_DIR').'/'.$target.'/release/raspiboard';
directory(dirname($binary));
copy($compiled, $binary);
process(['objcopy', '--only-keep-debug', $binary, '/output/debug/raspiboard.debug']);
process(['strip', '--strip-unneeded', $binary]);
process(['objcopy', '--add-gnu-debuglink=/output/debug/raspiboard.debug', $binary]);

foreach ([16, 32, 48, 64, 128, 256, 512] as $size) {
    $directory = $assets.'/icons/'.$size.'x'.$size.'/apps';
    directory($directory);
    process(['magick', 'img/icon_full.png', '-resize', $size.'x'.$size, $directory.'/'.$id.'.png']);
}
writeNotices($assets.'/notices');
$changelog = "raspiboard ($version-1) unstable; urgency=medium\n\n  * Package Raspiboard for Debian and Ubuntu desktops.\n\n -- Sergio Brighenti <sergio@brighenti.me>  ".date(DATE_RFC2822)."\n";
file_put_contents($assets.'/notices/changelog.Debian.gz', gzencode($changelog, 9));
process(['desktop-file-validate', 'assets/'.$id.'.desktop']);
file_put_contents('/output/build-info.json', json_encode(['version' => $version, 'format' => $format], JSON_THROW_ON_ERROR)."\n");
