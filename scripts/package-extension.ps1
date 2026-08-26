# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$OutputPath,
    [switch]$KeepDevelopmentKey,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repo "extension"
$manifestPath = Join-Path $source "manifest.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($manifest.version)) {
    throw "extension/manifest.json has no version"
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repo "dist/ghostlight-extension-v$($manifest.version).zip"
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
$tempRoot = Join-Path $tempBase ("ghostlight-extension-package-" + [guid]::NewGuid().ToString("N"))
$stage = Join-Path $tempRoot "stage"
New-Item -ItemType Directory -Path $stage | Out-Null

try {
    $rootFiles = @(
        "content.js",
        "manifest.json",
        "offscreen.html",
        "offscreen.js",
        "options.html",
        "options.js",
        "popup.html",
        "popup.js",
        "service-worker.js",
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
    foreach ($directory in @("lib", "vendor")) {
        Copy-Item -LiteralPath (Join-Path $source $directory) -Destination $stage -Recurse
    }
    $licenseDirectory = Join-Path $stage "licenses"
    New-Item -ItemType Directory -Path $licenseDirectory | Out-Null
    Copy-Item -LiteralPath (Join-Path $repo "LICENSE") `
        -Destination (Join-Path $licenseDirectory "Apache-2.0.txt")
    Copy-Item -LiteralPath (Join-Path $repo "docs/licenses/MIT.txt") `
        -Destination (Join-Path $licenseDirectory "MIT.txt")

    if (-not $KeepDevelopmentKey) {
        $stagedManifestPath = Join-Path $stage "manifest.json"
        $stagedManifest = Get-Content -LiteralPath $stagedManifestPath -Raw | ConvertFrom-Json
        [void]$stagedManifest.PSObject.Properties.Remove("key")
        # ConvertTo-Json separates lines with the platform newline, which made the
        # archive hash differ between Windows and Linux builds of identical source.
        # Pin CRLF: it is the exact serialization of the approved Chrome Web Store
        # revision (3570494f), so every host reproduces the reviewed bytes.
        $json = ($stagedManifest | ConvertTo-Json -Depth 20) -replace "\r?\n", "`r`n"
        [System.IO.File]::WriteAllText(
            $stagedManifestPath,
            $json + "`n",
            [System.Text.UTF8Encoding]::new($false)
        )
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::Open(
        $OutputPath,
        [System.IO.Compression.ZipArchiveMode]::Create
    )
    try {
        $fixedTimestamp = [System.DateTimeOffset]::Parse("2000-01-01T00:00:00Z")
        $stagedFiles = @(Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object {
            [System.IO.Path]::GetRelativePath($stage, $_.FullName).Replace("\", "/")
        })
        foreach ($stagedFile in $stagedFiles) {
            $entryName = [System.IO.Path]::GetRelativePath($stage, $stagedFile.FullName).Replace("\", "/")
            $entry = $archive.CreateEntry($entryName, [System.IO.Compression.CompressionLevel]::Optimal)
            $entry.LastWriteTime = $fixedTimestamp
            $inputStream = $stagedFile.OpenRead()
            $outputStream = $entry.Open()
            try {
                $inputStream.CopyTo($outputStream)
            }
            finally {
                $outputStream.Dispose()
                $inputStream.Dispose()
            }
        }
    }
    finally {
        $archive.Dispose()
    }

    $zip = [System.IO.Compression.ZipFile]::OpenRead($OutputPath)
    try {
        $names = @($zip.Entries | ForEach-Object { $_.FullName.Replace("\", "/") } | Sort-Object)
        if ($names -notcontains "manifest.json") {
            throw "extension ZIP does not contain manifest.json at its root"
        }
        foreach ($license in @("licenses/Apache-2.0.txt", "licenses/MIT.txt")) {
            if ($names -notcontains $license) {
                throw "extension ZIP does not contain $license"
            }
        }
        $forbidden = @($names | Where-Object {
            $_ -match '(^|/)(tests?|node_modules)(/|$)' -or
            $_ -match '(^|/)(package(-lock)?\.json|README\.md)$'
        })
        if ($forbidden.Count -gt 0) {
            throw "extension ZIP contains development files: $($forbidden -join ', ')"
        }

        $expectedNames = [System.Collections.Generic.List[string]]::new()
        foreach ($file in $rootFiles) {
            $expectedNames.Add($file)
        }
        foreach ($file in $iconFiles) {
            $expectedNames.Add("icons/$file")
        }
        foreach ($directory in @("lib", "vendor")) {
            Get-ChildItem -LiteralPath (Join-Path $source $directory) -File -Recurse |
                ForEach-Object {
                    $relative = [System.IO.Path]::GetRelativePath(
                        (Join-Path $source $directory),
                        $_.FullName
                    ).Replace("\", "/")
                    $expectedNames.Add("$directory/$relative")
                }
        }
        $expectedNames.Add("licenses/Apache-2.0.txt")
        $expectedNames.Add("licenses/MIT.txt")
        $expected = @($expectedNames | Sort-Object)
        if (($names -join "`n") -ne ($expected -join "`n")) {
            throw "extension ZIP contents differ from the explicit store package surface"
        }

        $manifestEntry = $zip.GetEntry("manifest.json")
        $reader = [System.IO.StreamReader]::new($manifestEntry.Open())
        try {
            $packagedManifest = $reader.ReadToEnd() | ConvertFrom-Json
        }
        finally {
            $reader.Dispose()
        }
        if ($packagedManifest.version -ne $manifest.version) {
            throw "extension ZIP manifest version differs from source"
        }
        if ($null -ne $packagedManifest.PSObject.Properties["key"]) {
            throw "extension ZIP contains the development key"
        }
    }
    finally {
        $zip.Dispose()
    }

    $hash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Output "Extension package: $OutputPath"
    Write-Output "Version: $($manifest.version)"
    Write-Output "SHA-256: $hash"
}
finally {
    $resolvedTemp = [System.IO.Path]::GetFullPath($tempRoot)
    if (-not $resolvedTemp.StartsWith([System.IO.Path]::GetFullPath($tempBase), [System.StringComparison]::OrdinalIgnoreCase) -or
        -not ([System.IO.Path]::GetFileName($resolvedTemp)).StartsWith("ghostlight-extension-package-")) {
        throw "Refusing to clean unexpected temporary path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp) {
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
    }
}
