<?php

declare(strict_types=1);

$root = dirname(__DIR__, 2);
@mkdir($root.'/test-results', 0755, true);
$workspace = $root.'/test-results/packaging '.bin2hex(random_bytes(5));
mkdir($workspace, 0755, true);

function check(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function command(array $args, ?array $environment = null): array
{
    $process = proc_open($args, [0 => ['file', '/dev/null', 'r'], 1 => ['pipe', 'w'], 2 => ['redirect', 1]], $pipes, env_vars: $environment);
    $output = stream_get_contents($pipes[1]);
    fclose($pipes[1]);
    return [proc_close($process), $output];
}

$tests = [
    'invalid job counts fail before container invocation' => function () use ($root): void {
        [$status, $output] = command([PHP_BINARY, $root.'/scripts/package-linux', '--jobs', '0']);
        check($status === 2 && str_contains($output, 'positive'), $output);
    },
    'unsupported engines are rejected' => function () use ($root): void {
        [$status, $output] = command([PHP_BINARY, $root.'/scripts/package-linux', '--engine', 'shell']);
        check($status === 2 && str_contains($output, 'podman or docker'), $output);
    },
    'unsupported package formats are rejected' => function () use ($root): void {
        [$status, $output] = command([PHP_BINARY, $root.'/scripts/package-linux', '--format', 'rpm']);
        check($status === 2 && str_contains($output, 'all, deb or appimage'), $output);
    },
    'missing tooling image is rejected' => function () use ($root): void {
        [$status, $output] = command([PHP_BINARY, $root.'/scripts/package-linux', '--image']);
        check($status === 2 && str_contains($output, 'image name'), $output);
    },
    'tooling image cannot consume another option' => function () use ($root): void {
        [$status, $output] = command([PHP_BINARY, $root.'/scripts/package-linux', '--image', '--json']);
        $result = json_decode($output, true, flags: JSON_THROW_ON_ERROR);
        check($status === 2 && $result['stage'] === 'arguments' && str_contains($result['message'], 'image name'), $output);
    },
    'failed build preserves downloads in a checkout with spaces' => function () use ($root, $workspace): void {
        $checkout = $workspace.'/checkout with spaces';
        mkdir($checkout.'/scripts', 0755, true);
        mkdir($checkout.'/packaging/linux', 0755, true);
        check(is_file($root.'/scripts/package-linux'), 'Missing packaging entrypoint');
        copy($root.'/scripts/package-linux', $checkout.'/scripts/package-linux');
        foreach (glob($root.'/packaging/linux/*.php') as $file) {
            copy($file, $checkout.'/packaging/linux/'.basename($file));
        }
        foreach (['Cargo.toml', 'Cargo.lock', 'LICENSE', 'README.md'] as $name) {
            copy($root.'/'.$name, $checkout.'/'.$name);
        }
        foreach (['src', 'assets', 'img', 'vendor', 'tests'] as $name) {
            mkdir($checkout.'/'.$name);
        }
        mkdir($checkout.'/target/previous', 0755, true);
        file_put_contents($checkout.'/target/previous/SHA256SUMS', "previous successful build\n");
        symlink('previous', $checkout.'/target/packages');
        mkdir($workspace.'/bin');
        file_put_contents($workspace.'/bin/podman', "#!/bin/sh\nexit 73\n");
        chmod($workspace.'/bin/podman', 0755);
        [$status, $output] = command(
            [PHP_BINARY, $checkout.'/scripts/package-linux', '--plain'],
            [...getenv(), 'PATH' => $workspace.'/bin:'.getenv('PATH')],
        );
        check($status === 73, $output);
        check(file_get_contents($checkout.'/target/packages/SHA256SUMS') === "previous successful build\n", 'Previous downloads changed');
        check(!str_contains($output, 'Packages ready'), 'Failure reported success');
    },
];

foreach ([false, true] as $overrides) {
    $tests['portable launcher '.($overrides ? 'preserves overrides' : 'resolves paths and arguments')] = function () use ($root, $workspace, $overrides): void {
        $appdir = $workspace.'/AppDir with spaces '.(int) $overrides;
        mkdir($appdir.'/usr/bin', 0755, true);
        check(is_file($root.'/packaging/linux/AppRun'), 'Missing portable launcher');
        copy($root.'/packaging/linux/AppRun', $appdir.'/AppRun');
        file_put_contents($appdir.'/usr/bin/raspiboard', "#!/usr/bin/env php\n<?php echo json_encode(['args'=>array_slice(\$argv,1),'env'=>getenv()], JSON_THROW_ON_ERROR);\n");
        chmod($appdir.'/usr/bin/raspiboard', 0755);
        $environment = array_filter(getenv(), static fn ($key) => !preg_match('/^(PIPEWIRE_|SPA_|APPDIR|LD_LIBRARY_PATH)/', $key), ARRAY_FILTER_USE_KEY);
        if ($overrides) {
            $environment += ['PIPEWIRE_MODULE_DIR' => '/custom/modules', 'SPA_PLUGIN_DIR' => '/custom/spa', 'PIPEWIRE_CONFIG_DIR' => '/custom/config', 'LD_LIBRARY_PATH' => '/custom/libs'];
        }
        [$status, $output] = command(['sh', $appdir.'/AppRun', 'a sound with spaces.wav', '--example'], $environment);
        check($status === 0, $output);
        $result = json_decode($output, true, flags: JSON_THROW_ON_ERROR);
        check($result['args'] === ['a sound with spaces.wav', '--example'], 'Arguments changed');
        check($result['env']['PIPEWIRE_MODULE_DIR'] === ($overrides ? '/custom/modules' : $appdir.'/usr/lib/pipewire-0.3'), 'Incorrect module path');
        check($result['env']['SPA_PLUGIN_DIR'] === ($overrides ? '/custom/spa' : $appdir.'/usr/lib/spa-0.2'), 'Incorrect SPA path');
        if ($overrides) {
            check($result['env']['PIPEWIRE_CONFIG_DIR'] === '/custom/config', 'Incorrect configuration path');
        }
        check($result['env']['LD_LIBRARY_PATH'] === $appdir.'/usr/lib'.($overrides ? ':/custom/libs' : ''), 'Incorrect library path');
    };
}

foreach (['mismatch', 'traversal'] as $kind) {
    $tests['artifact verifier rejects checksum '.$kind] = function () use ($root, $workspace, $kind): void {
        $directory = $workspace.'/checksum-'.$kind;
        mkdir($directory);
        file_put_contents($directory.'/example.deb', 'not a package');
        file_put_contents($directory.'/SHA256SUMS', str_repeat('0', 64).'  '.($kind === 'traversal' ? '../example.deb' : 'example.deb')."\n");
        [$status, $output] = command([PHP_BINARY, $root.'/packaging/linux/verify.php', $directory]);
        check($status !== 0 && str_contains($output, $kind === 'traversal' ? 'Invalid checksum entry' : 'Checksum mismatch'), $output);
    };
}

foreach ([
    [false, 'all', 'podman', null],
    [true, 'all', 'podman', null],
    [false, 'deb', 'podman', null],
    [false, 'appimage', 'podman', null],
    [true, 'deb', 'podman', null],
    [false, 'all', 'docker', 'example/tooling:cached'],
    [false, 'all', 'podman', 'example/tooling:cached'],
    [true, 'all', 'docker', 'example/tooling:cached'],
] as [$failBaseline, $format, $engine, $image]) {
    $label = $format.' '.$engine.($image === null ? '' : ' prebuilt');
    $tests[($failBaseline ? 'baseline failure preserves downloads: ' : 'successful build publishes selected format: ').$label] = function () use ($root, $workspace, $failBaseline, $format, $engine, $image, $label): void {
        $checkout = $workspace.'/completed checkout '.(int) $failBaseline.' '.$label;
        mkdir($checkout.'/scripts', 0755, true);
        mkdir($checkout.'/packaging/linux', 0755, true);
        copy($root.'/scripts/package-linux', $checkout.'/scripts/package-linux');
        copy($root.'/packaging/linux/common.php', $checkout.'/packaging/linux/common.php');
        foreach (['Cargo.toml', 'Cargo.lock', 'LICENSE', 'README.md'] as $name) {
            copy($root.'/'.$name, $checkout.'/'.$name);
        }
        foreach (['src', 'assets', 'img', 'vendor', 'tests'] as $name) {
            mkdir($checkout.'/'.$name);
        }
        mkdir($checkout.'/target/previous', 0755, true);
        file_put_contents($checkout.'/target/previous/SHA256SUMS', "previous\n");
        symlink('previous', $checkout.'/target/packages');
        $bin = $checkout.'/fake engine';
        mkdir($bin);
        file_put_contents($bin.'/'.$engine, <<<'PHP'
#!/usr/bin/env php
<?php
file_put_contents(getenv('PACKAGING_TEST_ENGINE_LOG'), json_encode(array_slice($argv, 1), JSON_THROW_ON_ERROR)."\n", FILE_APPEND);
echo "container diagnostics\n";
if (in_array('/source/packaging/linux/build.php', $argv, true)) {
    foreach ($argv as $index => $argument) {
        if ($argument === '--volume' && str_ends_with($argv[$index + 1], ':/output')) {
            $output = substr($argv[$index + 1], 0, -strlen(':/output'));
            if (!is_file($output.'/source/Cargo.toml')) {
                exit(31);
            }
            $format = 'all';
            foreach ($argv as $value) {
                if (str_starts_with($value, 'RASPIBOARD_PACKAGE_FORMAT=')) {
                    $format = substr($value, strlen('RASPIBOARD_PACKAGE_FORMAT='));
                }
            }
            $files = ['SHA256SUMS'];
            if ($format !== 'appimage') $files[] = 'raspiboard_0.1.0-1_amd64.deb';
            if ($format !== 'deb') $files[] = 'Raspiboard-0.1.0-x86_64.AppImage';
            foreach ($files as $name) {
                file_put_contents($output.'/artifacts/'.$name, "new downloads\n");
            }
        }
    }
} elseif (in_array('docker.io/library/ubuntu:26.04', $argv, true) && getenv('PACKAGING_TEST_FAIL_BASELINE')) {
    exit(74);
}
PHP
        );
        chmod($bin.'/'.$engine, 0755);
        $arguments = [PHP_BINARY, $checkout.'/scripts/package-linux', '--json', '--format', $format, '--engine', $engine];
        if ($image !== null) {
            array_push($arguments, '--image', $image);
        }
        $engineLog = $checkout.'/engine.jsonl';
        [$status, $output] = command(
            $arguments,
            [...getenv(), 'PATH' => $bin.':'.getenv('PATH'), 'PACKAGING_TEST_FAIL_BASELINE' => $failBaseline ? '1' : '', 'PACKAGING_TEST_ENGINE_LOG' => $engineLog],
        );
        $result = json_decode($output, true, flags: JSON_THROW_ON_ERROR);
        check(is_file($engineLog), $output);
        $calls = array_map(static fn ($line) => json_decode($line, true, flags: JSON_THROW_ON_ERROR), file($engineLog, FILE_IGNORE_NEW_LINES));
        $builds = array_filter($calls, static fn ($args) => $args[0] === 'build');
        check(count($builds) === ($image === null ? 1 : 0), 'Tooling image was built unexpectedly or not built');
        $packageCalls = array_values(array_filter($calls, static fn ($args) => in_array('/source/packaging/linux/build.php', $args, true)));
        check(count($packageCalls) === 1, 'Application must be built once');
        check(in_array($image ?? 'localhost/raspiboard-packaging:debian13', $packageCalls[0], true), 'Incorrect tooling image');
        $baselines = array_filter($calls, static fn ($args) => in_array('docker.io/library/debian:13-slim', $args, true) || in_array('docker.io/library/ubuntu:26.04', $args, true));
        check(count($baselines) === 2, 'Both compatibility baselines must be checked');
        if ($failBaseline) {
            check($status === 74 && $result['status'] === 'error', $output);
            check(file_get_contents($checkout.'/target/packages/SHA256SUMS') === "previous\n", 'Failed baseline replaced good downloads');
        } else {
            check($status === 0 && $result['status'] === 'ok', $output);
            check(file_get_contents($checkout.'/target/packages/SHA256SUMS') === "new downloads\n", 'Successful downloads not published');
            check(file_get_contents($checkout.'/target/previous/SHA256SUMS') === "previous\n", 'Previous build was destroyed');
            check(count($result['files']) === ($format === 'all' ? 3 : 2), $output);
            check($format !== 'deb' || glob($checkout.'/target/packages/*.AppImage') === [], 'Debian build published an AppImage');
            check($format !== 'appimage' || glob($checkout.'/target/packages/*.deb') === [], 'AppImage build published a Debian package');
        }
    };
}

$tests['concurrent builds cannot share the compilation cache'] = function () use ($root, $workspace): void {
    $checkout = $workspace.'/locked checkout';
    mkdir($checkout.'/scripts', 0755, true);
    mkdir($checkout.'/packaging/linux', 0755, true);
    mkdir($checkout.'/target/packaging/cache', 0755, true);
    copy($root.'/scripts/package-linux', $checkout.'/scripts/package-linux');
    copy($root.'/packaging/linux/common.php', $checkout.'/packaging/linux/common.php');
    $lock = fopen($checkout.'/target/packaging/cache.lock', 'c');
    flock($lock, LOCK_EX);
    try {
        [$status, $output] = command([PHP_BINARY, $checkout.'/scripts/package-linux', '--plain']);
        check($status === 2 && str_contains($output, 'Another packaging build'), $output);
    } finally {
        fclose($lock);
    }
};

if (is_executable('/usr/bin/pw-config')) {
    $tests['portable launcher preserves actual PipeWire user drop-ins'] = function () use ($root, $workspace): void {
        $appdir = $workspace.'/configuration probe';
        mkdir($appdir.'/usr/bin', 0755, true);
        mkdir($appdir.'/usr/share/pipewire', 0755, true);
        copy($root.'/packaging/linux/AppRun', $appdir.'/AppRun');
        file_put_contents($appdir.'/usr/share/pipewire/client.conf', "context.properties = { raspiboard.packaging.sentinel = bundled }\n");
        file_put_contents($appdir.'/usr/bin/raspiboard', "#!/bin/sh\nexec /usr/bin/pw-config -n client.conf merge context.properties\n");
        chmod($appdir.'/usr/bin/raspiboard', 0755);
        $home = $workspace.'/configuration home';
        mkdir($home.'/.config/pipewire/client.conf.d', 0755, true);
        file_put_contents($home.'/.config/pipewire/client.conf.d/99-packaging.conf', "context.properties = { raspiboard.packaging.sentinel = user-drop-in }\n");
        $environment = array_filter(getenv(), static fn ($key) => !preg_match('/^(PIPEWIRE_|SPA_|APPDIR|LD_LIBRARY_PATH)/', $key), ARRAY_FILTER_USE_KEY);
        [$status, $output] = command(['sh', $appdir.'/AppRun'], [...$environment, 'HOME' => $home, 'XDG_CONFIG_HOME' => $home.'/.config']);
        check($status === 0 && str_contains($output, 'user-drop-in'), 'Native PipeWire failed to load the user drop-in: '.$output);
    };
} else {
    echo "SKIP native PipeWire configuration check (runs in the packaging container)\n";
}

$tests['artifact version must match Cargo and both download names'] = function () use ($root): void {
    require_once $root.'/packaging/linux/verify.php';
    try {
        verifyVersion('0.1.0', '0.2.0-1', 'raspiboard_0.1.0-1_amd64.deb', 'Raspiboard-0.1.0-x86_64.AppImage');
    } catch (Throwable $error) {
        check(str_contains($error->getMessage(), 'Version mismatch'), $error->getMessage());
        verifyVersion('0.1.0', '0.1.0-1', 'raspiboard_0.1.0-1_amd64.deb', 'Raspiboard-0.1.0-x86_64.AppImage');
        return;
    }
    throw new RuntimeException('Mismatched version was accepted');
};

$tests['portable bundles reject host graphics libraries'] = function () use ($root, $workspace): void {
    $appdir = $workspace.'/graphics boundary';
    mkdir($appdir.'/usr/lib', 0755, true);
    file_put_contents($appdir.'/usr/lib/libwayland-client.so.0', 'incompatible display client');
    require_once $root.'/packaging/linux/verify.php';
    try {
        \Raspiboard\Packaging\verifyGraphicsBoundary($appdir);
    } catch (Throwable $error) {
        check(str_contains($error->getMessage(), 'Host graphics library bundled'), $error->getMessage());
        return;
    }
    throw new RuntimeException('Bundled host graphics library was accepted');
};

foreach (['deb' => 'sample.deb', 'appimage' => 'sample.AppImage'] as $format => $name) {
    $tests['checksum verification accepts only requested format: '.$format] = function () use ($root, $workspace, $format, $name): void {
        require_once $root.'/packaging/linux/verify.php';
        $directory = $workspace.'/checksums-'.$format;
        mkdir($directory);
        file_put_contents($directory.'/'.$name, 'package fixture');
        file_put_contents($directory.'/SHA256SUMS', hash_file('sha256', $directory.'/'.$name).'  '.$name."\n");
        verifyChecksums($directory, $format);
        $extra = $format === 'deb' ? 'extra.AppImage' : 'extra.deb';
        file_put_contents($directory.'/'.$extra, 'unexpected');
        try {
            verifyChecksums($directory, $format);
        } catch (RuntimeException) {
            return;
        }
        throw new RuntimeException('Unexpected package format accepted');
    };
}

$failures = 0;
foreach ($tests as $name => $test) {
    try {
        $test();
        echo "PASS $name\n";
    } catch (Throwable $error) {
        $failures++;
        fwrite(STDERR, "FAIL $name: {$error->getMessage()}\n");
    }
}
echo count($tests)." checks, $failures failures\n";
exit($failures === 0 ? 0 : 1);
