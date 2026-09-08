# SPDX-License-Identifier: Apache-2.0 OR MIT
# Verify the actual packager across distinct processes, including extracted bytes and modes.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$repo = Split-Path -Parent $PSScriptRoot
$area = Join-Path $repo (".tmp/portable-package/" + [guid]::NewGuid().ToString("N"))
$inputDirectory = Join-Path $area "input"
New-Item -ItemType Directory -Path $inputDirectory -Force | Out-Null
$expected = @{}
foreach ($name in @("ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector")) {
    $bytes = [System.Text.Encoding]::ASCII.GetBytes("portable fixture: $name`n")
    [System.IO.File]::WriteAllBytes((Join-Path $inputDirectory $name), $bytes)
    $expected[$name] = $bytes
}
foreach ($item in @(
    @{ source = "LICENSE"; name = "LICENSE" },
    @{ source = "docs/licenses/MIT.txt"; name = "MIT.txt" },
    @{ source = "LICENSING.md"; name = "LICENSING.md" }
)) {
    $expected[$item.name] = [System.IO.File]::ReadAllBytes((Join-Path $repo $item.source))
}
$archives = @()
foreach ($run in @("first", "second")) {
    # Separate pwsh processes are essential: a same-process run misses Pax's embedded PID.
    & (Join-Path $PSHOME $(if ($IsWindows) { "pwsh.exe" } else { "pwsh" })) -NoProfile -File `
        (Join-Path $repo "scripts/package-portable.ps1") -TargetTriple x86_64-unknown-linux-gnu `
        -BinaryDirectory $inputDirectory -OutputDirectory (Join-Path $area $run) -Version 1.2.3
    if ($LASTEXITCODE -ne 0) { throw "Portable packager failed: $run" }
    $archives += Join-Path $area "$run/ghostlight-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
}
$hashes = @($archives | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash })
if ($hashes[0] -ne $hashes[1]) { throw "Identical input produced different archive bytes" }
$stream = [System.IO.File]::OpenRead($archives[0])
$gzip = [System.IO.Compression.GZipStream]::new($stream, [System.IO.Compression.CompressionMode]::Decompress)
$reader = [System.Formats.Tar.TarReader]::new($gzip)
$seen = @{}
$directories = 0
try {
    while ($null -ne ($entry = $reader.GetNextEntry())) {
        if ($entry.Name -notmatch '^ghostlight-v1\.2\.3-x86_64-unknown-linux-gnu/') {
            throw "Unexpected archive path: $($entry.Name)"
        }
        if ($entry.Uid -ne 0 -or $entry.Gid -ne 0 -or $entry.ModificationTime.ToUnixTimeSeconds() -ne 946684800) {
            throw "Unexpected owner or timestamp: $($entry.Name)"
        }
        if ($entry.EntryType -eq [System.Formats.Tar.TarEntryType]::Directory) {
            if ([int]$entry.Mode -ne 493) { throw "Directory must have mode 0755" }
            $directories++
            continue
        }
        if ($entry.EntryType -ne [System.Formats.Tar.TarEntryType]::RegularFile) { throw "Unexpected archive entry type" }
        $name = [System.IO.Path]::GetFileName($entry.Name)
        if (-not $expected.ContainsKey($name) -or $seen.ContainsKey($name)) { throw "Unexpected or duplicate file: $name" }
        $seen[$name] = $true
        $mode = if ($name.StartsWith("ghostlight")) { 493 } else { 420 }
        if ([int]$entry.Mode -ne $mode) { throw "Incorrect executable/legal mode: $name" }
        $data = [System.IO.MemoryStream]::new()
        try {
            $entry.DataStream.CopyTo($data)
            if ([Convert]::ToBase64String($data.ToArray()) -ne [Convert]::ToBase64String($expected[$name])) {
                throw "Extracted file differs: $name"
            }
        } finally { $data.Dispose() }
    }
} finally { $reader.Dispose(); $gzip.Dispose(); $stream.Dispose() }
if ($directories -ne 1 -or $seen.Count -ne $expected.Count) { throw "Incomplete portable payload" }
Write-Output "PASS: separate-process reproducibility, exact payload, owners, timestamps, and executable modes"
Write-Output "Evidence: $area"
