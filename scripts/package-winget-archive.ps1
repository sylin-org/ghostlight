# SPDX-License-Identifier: Apache-2.0 OR MIT
# Build the Windows zip that the WinGet manifest installs from (ADR-0197 Decision 16). It is a
# carrier for `ghostlight install`, not a separate download channel: the edge, the service and the
# shell sit side by side, and the edge hands every command to the service beside it. It is
# deterministic: fixed entry order, fixed timestamps, one compression level.

param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryDirectory,
    [string]$OutputDirectory = "dist",
    [string]$Version,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$targetTriple = "x86_64-pc-windows-msvc"
$repo = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargo = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
    $Version = [regex]::Match($cargo, '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"').Groups["version"].Value
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw "Invalid WinGet archive version: $Version" }
$BinaryDirectory = [System.IO.Path]::GetFullPath($BinaryDirectory)
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
$baseName = "ghostlight-v$Version-$targetTriple"
$OutputPath = Join-Path $OutputDirectory "$baseName.zip"
if (Test-Path -LiteralPath $OutputPath) {
    if (-not $Force) { throw "Output exists: $OutputPath (pass -Force to replace it)" }
    Remove-Item -LiteralPath $OutputPath -Force
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$tempBase = [System.IO.Path]::GetTempPath()
$tempRoot = Join-Path $tempBase ("ghostlight-winget-archive-" + [guid]::NewGuid().ToString("N"))
$stage = Join-Path $tempRoot $baseName
New-Item -ItemType Directory -Path $stage -Force | Out-Null
try {
    foreach ($component in @("ghostlight", "ghostlight-service", "ghostlight-desktop")) {
        $source = Join-Path $BinaryDirectory "$component.exe"
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "WinGet archive is missing sibling binary: $source"
        }
        Copy-Item -LiteralPath $source -Destination (Join-Path $stage "$component.exe")
    }
    foreach ($legalFile in @(
        [ordered]@{ source = "LICENSE"; destination = "LICENSE" },
        [ordered]@{ source = "docs/licenses/MIT.txt"; destination = "MIT.txt" },
        [ordered]@{ source = "LICENSING.md"; destination = "LICENSING.md" }
    )) {
        Copy-Item -LiteralPath (Join-Path $repo $legalFile.source) -Destination (Join-Path $stage $legalFile.destination)
    }

    $timestamp = [System.DateTimeOffset]::Parse("2000-01-01T00:00:00Z")
    $files = @(Get-ChildItem -LiteralPath $stage -File | Sort-Object Name)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::Open($OutputPath, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in $files) {
            $entry = $archive.CreateEntry("$baseName/$($file.Name)", [System.IO.Compression.CompressionLevel]::Optimal)
            $entry.LastWriteTime = $timestamp
            $input = $file.OpenRead()
            $output = $entry.Open()
            try { $input.CopyTo($output) }
            finally { $output.Dispose(); $input.Dispose() }
        }
    }
    finally { $archive.Dispose() }
    Write-Output "WinGet archive: $OutputPath"
    Write-Output "SHA-256: $((Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant())"
}
finally {
    $resolved = [System.IO.Path]::GetFullPath($tempRoot)
    if (-not $resolved.StartsWith([System.IO.Path]::GetFullPath($tempBase)) -or
        -not [System.IO.Path]::GetFileName($resolved).StartsWith("ghostlight-winget-archive-")) {
        throw "Refusing to clean unexpected WinGet staging path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
