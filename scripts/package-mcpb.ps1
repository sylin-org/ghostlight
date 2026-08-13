# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$CandidateDirectory,
    [string]$ArtifactRoot,
    [string]$Version,
    [string]$OutputPath,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($CandidateDirectory) -eq [string]::IsNullOrWhiteSpace($ArtifactRoot)) {
    throw "Provide exactly one of -CandidateDirectory or -ArtifactRoot"
}
$artifactMode = -not [string]::IsNullOrWhiteSpace($ArtifactRoot)
if ($artifactMode) {
    $ArtifactRoot = [System.IO.Path]::GetFullPath($ArtifactRoot)
    if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
        throw "Artifact preparation requires a three-part -Version"
    }
} else {
    $CandidateDirectory = [System.IO.Path]::GetFullPath($CandidateDirectory)
    $candidate = Get-Content -LiteralPath (Join-Path $CandidateDirectory "release-candidate.json") -Raw | ConvertFrom-Json
    & (Join-Path $PSScriptRoot "check-release-candidate.ps1") `
        -CandidateDirectory $CandidateDirectory `
        -ExpectedStatus $candidate.status
    $Version = $candidate.version
}
$source = Join-Path $repo "packaging/mcpb"
$manifest = Get-Content -LiteralPath (Join-Path $source "manifest.json") -Raw | ConvertFrom-Json
if ($manifest.manifest_version -ne "0.3" -or $manifest.version -ne $Version) {
    throw "MCPB manifest does not match release version $Version"
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repo "dist/ghostlight-v$Version.mcpb"
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
if (Test-Path -LiteralPath $OutputPath) {
    if (-not $Force) {
        throw "Output already exists: $OutputPath (pass -Force to replace it)"
    }
    Remove-Item -LiteralPath $OutputPath -Force
}
New-Item -ItemType Directory -Path (Split-Path -Parent $OutputPath) -Force | Out-Null

$tempBase = [System.IO.Path]::GetTempPath()
$tempRoot = Join-Path $tempBase ("ghostlight-mcpb-package-" + [guid]::NewGuid().ToString("N"))
$stage = Join-Path $tempRoot "stage"
New-Item -ItemType Directory -Path (Join-Path $stage "server") -Force | Out-Null
try {
    foreach ($file in @("manifest.json", "README.md", "icon.png")) {
        Copy-Item -LiteralPath (Join-Path $source $file) -Destination (Join-Path $stage $file)
    }
    Copy-Item -LiteralPath (Join-Path $source "server/launch.js") -Destination (Join-Path $stage "server/launch.js")
    foreach ($legalFile in @(
        [ordered]@{ source = "LICENSE"; destination = "LICENSE" },
        [ordered]@{ source = "docs/licenses/MIT.txt"; destination = "MIT.txt" },
        [ordered]@{ source = "docs/licenses/LicenseRef-Ghostlight-Commercial.txt"; destination = "LicenseRef-Ghostlight-Commercial.txt" },
        [ordered]@{ source = "LICENSING.md"; destination = "LICENSING.md" }
    )) {
        Copy-Item -LiteralPath (Join-Path $repo $legalFile.source) -Destination (Join-Path $stage $legalFile.destination)
    }
    foreach ($target in @(
        [ordered]@{ name = "x86_64-pc-windows-msvc"; extension = ".exe" },
        [ordered]@{ name = "aarch64-apple-darwin"; extension = "" },
        [ordered]@{ name = "x86_64-apple-darwin"; extension = "" }
    )) {
        $targetDirectory = Join-Path $stage "server/bin/$($target.name)"
        New-Item -ItemType Directory -Path $targetDirectory -Force | Out-Null
        foreach ($component in @("ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector")) {
            $asset = "$component-$($target.name)$($target.extension)"
            $sourcePath = if ($artifactMode) {
                $found = @(Get-ChildItem -LiteralPath $ArtifactRoot -Recurse -File -Filter $asset)
                if ($found.Count -ne 1) { throw "Expected one MCPB binary $asset, found $($found.Count)" }
                $found[0].FullName
            } else {
                Join-Path $CandidateDirectory "assets/$asset"
            }
            if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
                throw "Release input is missing MCPB binary $asset"
            }
            Copy-Item -LiteralPath $sourcePath -Destination (Join-Path $targetDirectory "$component$($target.extension)")
        }
    }

    & node --test (Join-Path $source "test/launcher.test.js")
    if ($LASTEXITCODE -ne 0) {
        throw "MCPB launcher tests failed"
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::Open($OutputPath, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        $timestamp = [System.DateTimeOffset]::Parse("2000-01-01T00:00:00Z")
        $files = @(Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object {
            [System.IO.Path]::GetRelativePath($stage, $_.FullName).Replace("\", "/")
        })
        foreach ($file in $files) {
            $name = [System.IO.Path]::GetRelativePath($stage, $file.FullName).Replace("\", "/")
            $entry = $archive.CreateEntry($name, [System.IO.Compression.CompressionLevel]::Optimal)
            $entry.LastWriteTime = $timestamp
            $input = $file.OpenRead()
            $output = $entry.Open()
            try { $input.CopyTo($output) }
            finally { $output.Dispose(); $input.Dispose() }
        }
    }
    finally { $archive.Dispose() }

    $zip = [System.IO.Compression.ZipFile]::OpenRead($OutputPath)
    try {
        $names = @($zip.Entries | ForEach-Object FullName)
        foreach ($required in @(
            "manifest.json",
            "server/launch.js",
            "server/bin/x86_64-pc-windows-msvc/ghostlight.exe",
            "server/bin/aarch64-apple-darwin/ghostlight",
            "server/bin/x86_64-apple-darwin/ghostlight",
            "LICENSE",
            "MIT.txt",
            "LicenseRef-Ghostlight-Commercial.txt",
            "LICENSING.md"
        )) {
            if ($names -notcontains $required) { throw "MCPB is missing $required" }
        }
    }
    finally { $zip.Dispose() }
    Write-Output "MCPB package: $OutputPath"
    Write-Output "SHA-256: $((Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant())"
}
finally {
    $resolved = [System.IO.Path]::GetFullPath($tempRoot)
    if (-not $resolved.StartsWith([System.IO.Path]::GetFullPath($tempBase)) -or
        -not [System.IO.Path]::GetFileName($resolved).StartsWith("ghostlight-mcpb-package-")) {
        throw "Refusing to clean unexpected MCPB staging path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
