<?php

declare(strict_types=1);

$tools = json_decode(file_get_contents(__DIR__.'/tools.json'), true, flags: JSON_THROW_ON_ERROR);
mkdir('/opt/downloads', 0755, true);
foreach ($tools as $name => $tool) {
    $path = '/opt/downloads/'.$name;
    $process = proc_open(
        ['curl', '--fail', '--location', '--retry', '3', '--output', $path, $tool['url']],
        [STDIN, STDOUT, STDERR],
        $pipes,
    );
    if (!is_resource($process) || proc_close($process) !== 0) {
        throw new RuntimeException('Download failed: '.$name);
    }
    if (!hash_equals($tool['sha256'], hash_file('sha256', $path))) {
        throw new RuntimeException('SHA-256 mismatch: '.$name.'; review the upstream release before updating tools.json');
    }
}
