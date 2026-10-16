# SPDX-License-Identifier: Apache-2.0 OR MIT
# Verify the actual WinGet archive builder across distinct processes, including extracted bytes.
# It packages fixture files only and starts no Ghostlight executable.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$repo = Split-Path -Parent $PSScriptRoot
$area = Join-Path $repo (".tmp/winget-archive/" + [guid]::NewGuid().ToString("N"))
$inputDirectory = Join-Path $area "input"
New-Item -ItemType Directory -Path $inputDirectory -Force | Out-Null
$expected = @{}
foreach ($name in @("ghostlight", "ghostlight-service")) {
    $bytes = [System.Text.Encoding]::ASCII.GetBytes("archive fixture: $name`n")
    [System.IO.File]::WriteAllBytes((Join-Path $inputDirectory "$name.exe"), $bytes)
    $expected["$name.exe"] = $bytes
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
    # Separate pwsh processes catch process-dependent bytes that one process would miss.
    & (Join-Path $PSHOME $(if ($IsWindows) { "pwsh.exe" } else { "pwsh" })) -NoProfile -File `
        (Join-Path $repo "scripts/package-winget-archive.ps1") `
        -BinaryDirectory $inputDirectory -OutputDirectory (Join-Path $area $run) -Version 1.2.3
    if ($LASTEXITCODE -ne 0) { throw "WinGet archive builder failed: $run" }
    $archives += Join-Path $area "$run/ghostlight-v1.2.3-x86_64-pc-windows-msvc.zip"
}
$hashes = @($archives | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash })
if ($hashes[0] -ne $hashes[1]) { throw "Identical input produced different archive bytes" }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($archives[0])
$seen = @{}
# A ZIP entry stores DOS local clock fields, so compare the stored clock, not an instant.
$fixedClock = [datetime]::new(2000, 1, 1, 0, 0, 0)
try {
    foreach ($entry in $zip.Entries) {
        if ($entry.FullName -notmatch '^ghostlight-v1\.2\.3-x86_64-pc-windows-msvc/[^/]+$') {
            throw "Unexpected archive path: $($entry.FullName)"
        }
        if ($entry.LastWriteTime.DateTime -ne $fixedClock) {
            throw "Unexpected timestamp: $($entry.FullName)"
        }
        $name = $entry.Name
        if (-not $expected.ContainsKey($name) -or $seen.ContainsKey($name)) { throw "Unexpected or duplicate file: $name" }
        $seen[$name] = $true
        $data = [System.IO.MemoryStream]::new()
        $stream = $entry.Open()
        try {
            $stream.CopyTo($data)
            if ([Convert]::ToBase64String($data.ToArray()) -ne [Convert]::ToBase64String($expected[$name])) {
                throw "Extracted file differs: $name"
            }
        } finally { $stream.Dispose(); $data.Dispose() }
    }
} finally { $zip.Dispose() }
if ($seen.Count -ne $expected.Count) { throw "Incomplete WinGet archive payload" }
Write-Output "PASS: separate-process reproducibility, exact payload, and timestamps"
Write-Output "Evidence: $area"
