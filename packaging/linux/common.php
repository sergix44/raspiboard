<?php

declare(strict_types=1);

namespace Raspiboard\Packaging;

function process(array $arguments, ?string $directory = null, ?array $environment = null, bool $capture = false, ?string $log = null): string
{
    $stdout = $log !== null ? ['file', $log, 'a'] : ($capture ? ['pipe', 'w'] : STDOUT);
    $stderr = $log !== null ? ['redirect', 1] : STDERR;
    $child = proc_open($arguments, [0 => ['file', '/dev/null', 'r'], 1 => $stdout, 2 => $stderr], $pipes, $directory, $environment);
    if (!is_resource($child)) {
        throw new \RuntimeException('Could not start '.$arguments[0], 1);
    }
    $output = '';
    if ($capture && $log === null) {
        $output = stream_get_contents($pipes[1]);
        fclose($pipes[1]);
    }
    $status = proc_close($child);
    if ($status !== 0) {
        throw new \RuntimeException($arguments[0].' failed'.($log !== null ? '; see '.$log : ''), $status > 0 ? $status : 1);
    }
    return $output;
}

function directory(string $path): void
{
    if (!is_dir($path) && !mkdir($path, 0755, true) && !is_dir($path)) {
        throw new \RuntimeException('Cannot create '.$path);
    }
}

function copyTree(string $source, string $destination): void
{
    if (is_link($source)) {
        throw new \RuntimeException('Source snapshots cannot contain symlinks: '.$source);
    }
    if (is_dir($source)) {
        directory($destination);
        foreach (new \FilesystemIterator($source) as $entry) {
            if (!in_array($entry->getFilename(), ['target', '.git', '.aws', '.codex', '.agents', '__pycache__'], true)) {
                copyTree($entry->getPathname(), $destination.'/'.$entry->getFilename());
            }
        }
    } elseif (!copy($source, $destination)) {
        throw new \RuntimeException('Cannot copy '.$source);
    } else {
        chmod($destination, fileperms($source) & 0777);
        touch($destination, filemtime($source));
    }
}
