# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$OutputPath,
    [switch]$MakeXpi,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repo "extension-firefox"
$manifestPath = Join-Path $source "manifest.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($manifest.version)) {
    throw "extension-firefox/manifest.json has no version"
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repo "dist/ghostlight-firefox-extension-v$($manifest.version).zip"
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
if (Test-Path -LiteralPath $OutputPath) {
    if (-not $Force) {
        throw "Output already exists: $OutputPath (pass -Force to replace this exact file)"
    }
    Remove-Item -LiteralPath $OutputPath -Force
}
$outputParent = Split-Path -Parent $OutputPath
if (-not (Test-Path -LiteralPath $outputParent)) {
    New-Item -ItemType Directory -Path $outputParent | Out-Null
}

$tempBase = [System.IO.Path]::GetTempPath()
$tempRoot = Join-Path $tempBase ("ghostlight-firefox-package-" + [guid]::NewGuid().ToString("N"))
$stage = Join-Path $tempRoot "stage"
New-Item -ItemType Directory -Path $stage | Out-Null

try {
    $rootFiles = @(
        "manifest.json",
        "background.js",
        "options.html",
        "options.js",
        "popup.html",
        "popup.js",
        "setup.html",
        "ui.css"
    )
    $iconFiles = @(
        "icon16.png",
        "icon32.png",
        "icon48.png",
        "icon128.png"
    )
    foreach ($file in $rootFiles) {
        $from = Join-Path $source $file
        if (-not (Test-Path -LiteralPath $from)) {
            throw "Required extension file is missing: $file"
        }
        Copy-Item -LiteralPath $from -Destination (Join-Path $stage $file)
    }
    $stagedIcons = Join-Path $stage "icons"
    New-Item -ItemType Directory -Path $stagedIcons | Out-Null
    foreach ($file in $iconFiles) {
        $from = Join-Path (Join-Path $source "icons") $file
        if (-not (Test-Path -LiteralPath $from -PathType Leaf)) {
            throw "Required extension icon is missing: $file"
        }
        Copy-Item -LiteralPath $from -Destination (Join-Path $stagedIcons $file)
    }
    foreach ($directory in @("lib")) {
        Copy-Item -LiteralPath (Join-Path $source $directory) -Destination $stage -Recurse
    }
    $licenseDirectory = Join-Path $stage "licenses"
    New-Item -ItemType Directory -Path $licenseDirectory | Out-Null
    Copy-Item -LiteralPath (Join-Path $repo "LICENSE") `
        -Destination (Join-Path $licenseDirectory "Apache-2.0.txt")
    Copy-Item -LiteralPath (Join-Path $repo "docs/licenses/MIT.txt") `
        -Destination (Join-Path $licenseDirectory "MIT.txt")

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem

    $fixedTimestamp = [DateTimeOffset]::new(2026, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
    $zipFile = [System.IO.File]::Open(
        $OutputPath,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::ReadWrite,
        [System.IO.FileShare]::None
    )
    try {
        $archive = [System.IO.Compression.ZipArchive]::new(
            $zipFile,
            [System.IO.Compression.ZipArchiveMode]::Create,
            $false,
            [System.Text.UTF8Encoding]::new($false)
        )
        try {
            $stagedItems = Get-ChildItem -LiteralPath $stage -Recurse -File |
                Sort-Object {
                    $_.FullName.Substring($stage.Length + 1).Replace("\", "/")
                }
            foreach ($item in $stagedItems) {
                $relative = $item.FullName.Substring($stage.Length + 1).Replace("\", "/")
                $entry = $archive.CreateEntry($relative, [System.IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = $fixedTimestamp
                $entryStream = $entry.Open()
                try {
                    $sourceStream = [System.IO.File]::OpenRead($item.FullName)
                    try {
                        $sourceStream.CopyTo($entryStream)
                    } finally {
                        $sourceStream.Dispose()
                    }
                } finally {
                    $entryStream.Dispose()
                }
            }
        } finally {
            $archive.Dispose()
        }
    } finally {
        $zipFile.Dispose()
    }

    $hash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash
    $size = (Get-Item -LiteralPath $OutputPath).Length
    Write-Host ("Packaged {0} ({1} bytes, SHA-256: {2})" -f $OutputPath, $size, $hash)

    if ($MakeXpi) {
        $xpiPath = [System.IO.Path]::ChangeExtension($OutputPath, ".xpi")
        Copy-Item -LiteralPath $OutputPath -Destination $xpiPath -Force
        Write-Host ("Created XPI: {0}" -f $xpiPath)
    }
} finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
