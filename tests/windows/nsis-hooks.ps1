# SPDX-License-Identifier: Apache-2.0 OR MIT
# Exercise real NSIS hook execution without installing, unregistering, or deleting a product.
[CmdletBinding()]
param(
    [string]$MakeNsis = $env:GHOSTLIGHT_MAKENSIS,
    [string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'The NSIS hook contract runs on Windows.' }
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if (-not $MakeNsis) {
    $MakeNsis = Join-Path $env:LOCALAPPDATA 'tauri/NSIS/makensis.exe'
}
if (-not (Test-Path -LiteralPath $MakeNsis -PathType Leaf)) {
    throw 'Set GHOSTLIGHT_MAKENSIS to the installed NSIS compiler executable.'
}
$artifactRoot = [IO.Path]::GetFullPath((Join-Path $repository '.tmp'))
if (-not $EvidenceDirectory) {
    $EvidenceDirectory = Join-Path $artifactRoot "nsis-hooks-$PID-$([DateTime]::UtcNow.ToString('yyyyMMddHHmmss'))"
}
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
if (-not $evidence.StartsWith($artifactRoot + [IO.Path]::DirectorySeparatorChar,
        [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Fixture evidence must be a new directory under this repository .tmp.'
}
if (Test-Path -LiteralPath $evidence) { throw 'Preserve existing evidence; choose a new directory.' }
New-Item -ItemType Directory -Path $evidence | Out-Null
$hooks = Join-Path $repository 'crates/orchestrator/packaging/windows/hooks.nsh'
$fixture = Join-Path $PSScriptRoot 'nsis-hooks.nsi'
$executable = Join-Path $evidence 'hook-contract.exe'
$compileLog = Join-Path $evidence 'compile.log'
& $MakeNsis '/V2' "/DGHOSTLIGHT_TEST_EXE=$executable" "/DGHOSTLIGHT_TEST_ROOT=$evidence/unused" `
    "/DGHOSTLIGHT_HOOKS=$hooks" $fixture *> $compileLog
if ($LASTEXITCODE -ne 0) { throw "NSIS compilation failed; see $compileLog" }

$before = @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -in @('ghostlight.exe', 'ghostlight-mcp-connector.exe', 'ghostlight-browser-connector.exe')
} | Select-Object ProcessId, CreationDate, ExecutablePath)
$results = [Collections.Generic.List[object]]::new()
try {
    foreach ($case in @('empty', 'foreign-lock', 'invalid-sibling', 'abort-after-acquire')) {
        $directory = Join-Path $evidence $case
        New-Item -ItemType Directory -Path $directory | Out-Null
        $lock = Join-Path $directory 'deploy.lock'
        if ($case -eq 'foreign-lock') { [IO.File]::WriteAllText($lock, 'other deployment') }
        if ($case -eq 'invalid-sibling') {
            New-Item -ItemType Directory -Path (Join-Path $directory 'ghostlight.exe') | Out-Null
        }
        $arguments = @('/S')
        if ($case -eq 'abort-after-acquire') { $arguments += '/FAIL_AFTER_ACQUIRE' }
        # NSIS requires /D last and accepts its unquoted remainder, including spaces.
        $arguments += "/D=$directory"
        $process = Start-Process -FilePath $executable -ArgumentList $arguments -WindowStyle Hidden -PassThru
        if (-not $process.WaitForExit(15000)) {
            throw "Hook fixture timed out in $case; evidence and process $($process.Id) retained."
        }
        $expected = switch ($case) { 'empty' { 0 } 'abort-after-acquire' { 23 } default { 1 } }
        $lockExists = Test-Path -LiteralPath $lock
        $prepared = Test-Path -LiteralPath (Join-Path $directory 'prepared.txt')
        $passed = $process.ExitCode -eq $expected -and $prepared -eq ($case -eq 'empty')
        if ($case -eq 'foreign-lock') {
            $passed = $passed -and $lockExists -and [IO.File]::ReadAllText($lock) -eq 'other deployment'
        } else { $passed = $passed -and -not $lockExists }
        $results.Add(@{ case=$case; exit=$process.ExitCode; expected=$expected; lock=$lockExists;
            prepared=$prepared; passed=$passed })
        if (-not $passed) { throw "Hook contract failed in $case; retained $evidence" }
    }
    foreach ($original in $before) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($original.ProcessId)"
        if (-not $current -or $current.CreationDate -ne $original.CreationDate) {
            throw "Unrelated Ghostlight process $($original.ProcessId) did not survive the fixture."
        }
    }
    Write-Output "PASS: four NSIS install-hook cases; $($before.Count) unrelated Ghostlight processes survived."
} finally {
    @{ source=(git -C $repository rev-parse HEAD); hooks_sha256=(Get-FileHash $hooks).Hash;
        executable_sha256=(Get-FileHash $executable).Hash; cases=$results.ToArray();
        unrelated_before=$before; scope='install-hook component fixture; no installed package or uninstall executed' } |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidence 'report.json') -Encoding utf8
    Write-Output "Evidence: $evidence"
}
