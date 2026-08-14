# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet(
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu"
    )]
    [string]$TargetTriple,
    [Parameter(Mandatory = $true)]
    [string]$BinaryDirectory,
    [string]$OutputDirectory = "dist",
    [string]$Version,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargo = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
    $Version = [regex]::Match($cargo, '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"').Groups["version"].Value
}
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw "Invalid portable package version: $Version" }
$BinaryDirectory = [System.IO.Path]::GetFullPath($BinaryDirectory)
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
$windows = $TargetTriple -eq "x86_64-pc-windows-msvc"
$extension = if ($windows) { ".exe" } else { "" }
$formatExtension = if ($windows) { ".zip" } else { ".tar.gz" }
$baseName = "ghostlight-v$Version-$TargetTriple"
$OutputPath = Join-Path $OutputDirectory "$baseName$formatExtension"
if (Test-Path -LiteralPath $OutputPath) {
    if (-not $Force) { throw "Output exists: $OutputPath (pass -Force to replace it)" }
    Remove-Item -LiteralPath $OutputPath -Force
}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$tempBase = [System.IO.Path]::GetTempPath()
$tempRoot = Join-Path $tempBase ("ghostlight-portable-" + [guid]::NewGuid().ToString("N"))
$stage = Join-Path $tempRoot $baseName
New-Item -ItemType Directory -Path $stage -Force | Out-Null
try {
    foreach ($component in @("ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector")) {
        $source = Join-Path $BinaryDirectory "$component$extension"
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Portable package is missing sibling binary: $source"
        }
        Copy-Item -LiteralPath $source -Destination (Join-Path $stage "$component$extension")
    }
    foreach ($legalFile in @(
        [ordered]@{ source = "LICENSE"; destination = "LICENSE" },
        [ordered]@{ source = "docs/licenses/MIT.txt"; destination = "MIT.txt" },
        [ordered]@{ source = "docs/licenses/LicenseRef-Ghostlight-Commercial.txt"; destination = "LicenseRef-Ghostlight-Commercial.txt" },
        [ordered]@{ source = "LICENSING.md"; destination = "LICENSING.md" }
    )) {
        Copy-Item -LiteralPath (Join-Path $repo $legalFile.source) -Destination (Join-Path $stage $legalFile.destination)
    }

    $timestamp = [System.DateTimeOffset]::Parse("2000-01-01T00:00:00Z")
    $files = @(Get-ChildItem -LiteralPath $stage -File | Sort-Object Name)
    if ($windows) {
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
    } else {
        $fileStream = [System.IO.File]::Create($OutputPath)
        $gzip = [System.IO.Compression.GZipStream]::new(
            $fileStream,
            [System.IO.Compression.CompressionLevel]::SmallestSize,
            $true
        )
        $tar = [System.Formats.Tar.TarWriter]::new($gzip, [System.Formats.Tar.TarEntryFormat]::Pax, $true)
        try {
            $directoryEntry = [System.Formats.Tar.PaxTarEntry]::new(
                [System.Formats.Tar.TarEntryType]::Directory,
                "$baseName/"
            )
            $directoryEntry.ModificationTime = $timestamp
            $directoryEntry.Mode = [System.IO.UnixFileMode]493
            $tar.WriteEntry($directoryEntry)
            foreach ($file in $files) {
                $entry = [System.Formats.Tar.PaxTarEntry]::new(
                    [System.Formats.Tar.TarEntryType]::RegularFile,
                    "$baseName/$($file.Name)"
                )
                $entry.ModificationTime = $timestamp
                $entry.Mode = if ($file.Name -in @(
                    "ghostlight",
                    "ghostlight-mcp-connector",
                    "ghostlight-browser-connector"
                )) { [System.IO.UnixFileMode]493 } else { [System.IO.UnixFileMode]420 }
                $entry.DataStream = $file.OpenRead()
                try { $tar.WriteEntry($entry) }
                finally { $entry.DataStream.Dispose() }
            }
        }
        finally {
            $tar.Dispose()
            $gzip.Dispose()
            $fileStream.Dispose()
        }
    }
    Write-Output "Portable package: $OutputPath"
    Write-Output "SHA-256: $((Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant())"
}
finally {
    $resolved = [System.IO.Path]::GetFullPath($tempRoot)
    if (-not $resolved.StartsWith([System.IO.Path]::GetFullPath($tempBase)) -or
        -not [System.IO.Path]::GetFileName($resolved).StartsWith("ghostlight-portable-")) {
        throw "Refusing to clean unexpected portable staging path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
