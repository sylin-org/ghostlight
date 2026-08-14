# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$BaselinePath = "docs/repository-integrity-baseline.json"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$baseline = Get-Content -LiteralPath (Join-Path $repo $BaselinePath) -Raw | ConvertFrom-Json
if ($baseline.schemaVersion -ne 1) {
    throw "Unsupported repository integrity baseline"
}

$tracked = @(& git -C $repo ls-files)
if ($LASTEXITCODE -ne 0 -or $tracked.Count -eq 0) {
    throw "Could not enumerate tracked files"
}

$textExtensions = @(
    ".bat", ".cjs", ".cmd", ".css", ".editorconfig", ".gitattributes", ".gitignore",
    ".html", ".js", ".json", ".jsonc", ".lock", ".md", ".mjs", ".nsh", ".ps1",
    ".rb", ".rs", ".sh", ".svg", ".toml", ".txt", ".xml", ".yaml", ".yml"
)
$textNames = @("CODEOWNERS", "LICENSE", "NOTICE", "SECURITY")
$nonAscii = [System.Collections.Generic.List[string]]::new()
$nulFiles = [System.Collections.Generic.List[string]]::new()

foreach ($relative in $tracked) {
    $path = Join-Path $repo $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Tracked file is unreadable or missing: $relative"
    }
    $item = Get-Item -LiteralPath $path -Force
    $isText = $textExtensions -contains $item.Extension.ToLowerInvariant() -or
        $textNames -contains $item.Name
    if (-not $isText) {
        continue
    }
    $bytes = [System.IO.File]::ReadAllBytes($item.FullName)
    if ($bytes -contains 0) {
        [void]$nulFiles.Add($relative)
    }
    if ($null -ne ($bytes | Where-Object { $_ -gt 127 } | Select-Object -First 1)) {
        [void]$nonAscii.Add($relative.Replace("\", "/"))
    }
}

if ($nulFiles.Count -gt 0) {
    throw "Text files contain NUL bytes: $($nulFiles -join ', ')"
}

$expectedNonAscii = @($baseline.nonAsciiHistoricalFiles | Sort-Object)
$observedNonAscii = @($nonAscii | Sort-Object)
if (($expectedNonAscii -join "`n") -ne ($observedNonAscii -join "`n")) {
    $added = @($observedNonAscii | Where-Object { $expectedNonAscii -notcontains $_ })
    $removed = @($expectedNonAscii | Where-Object { $observedNonAscii -notcontains $_ })
    throw "Non-ASCII historical baseline drifted. Added: $($added -join ', '); removed: $($removed -join ', ')"
}

$brokenLinks = [System.Collections.Generic.List[string]]::new()
$linkPattern = [regex]'!?\[[^\]]*\]\((?<target>[^)]+)\)'
foreach ($relative in @($tracked | Where-Object { $_.EndsWith(".md") })) {
    $path = Join-Path $repo $relative
    $directory = Split-Path -Parent $path
    $insideFence = $false
    $lineNumber = 0
    foreach ($line in Get-Content -LiteralPath $path) {
        $lineNumber += 1
        if ($line -match '^\s*(```|~~~)') {
            $insideFence = -not $insideFence
            continue
        }
        if ($insideFence) {
            continue
        }
        if ($line -match '^(    |\t)') {
            continue
        }
        $withoutInlineCode = [regex]::Replace($line, '`[^`]*`', '')
        foreach ($match in $linkPattern.Matches($withoutInlineCode)) {
            $target = $match.Groups["target"].Value.Trim().Trim("<", ">")
            if ($target -match '^(https?://|mailto:|#|data:)' -or
                $target -match '[{}*]' -or
                [string]::IsNullOrWhiteSpace($target)) {
                continue
            }
            $target = ($target -split '\s+["'']', 2)[0]
            $localPath = (($target -split '#', 2)[0] -replace '%20', ' ')
            if ([string]::IsNullOrWhiteSpace($localPath)) {
                continue
            }
            $resolved = if ([System.IO.Path]::IsPathRooted($localPath)) {
                [System.IO.Path]::GetFullPath($localPath)
            }
            else {
                [System.IO.Path]::GetFullPath((Join-Path $directory $localPath))
            }
            if (-not (Test-Path -LiteralPath $resolved)) {
                [void]$brokenLinks.Add("$relative`:$lineNumber -> $target")
            }
        }
    }
}
if ($brokenLinks.Count -gt 0) {
    throw "Broken local documentation links:`n$($brokenLinks -join "`n")"
}

$cargo = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
$cargoMatch = [regex]::Match(
    $cargo,
    '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"'
)
if (-not $cargoMatch.Success) {
    throw "Could not read the workspace version"
}
$sourceVersion = $cargoMatch.Groups["version"].Value
$extension = Get-Content -LiteralPath (Join-Path $repo "extension/manifest.json") -Raw |
    ConvertFrom-Json
$tauri = Get-Content -LiteralPath (Join-Path $repo "crates/orchestrator/tauri.conf.json") -Raw |
    ConvertFrom-Json
if ($extension.version -ne $sourceVersion -or $tauri.version -ne $sourceVersion) {
    throw "Source, extension, and desktop versions differ"
}

Write-Output "Repository integrity: $($tracked.Count) tracked files readable; local links valid; source version $sourceVersion aligned."
Write-Output "Historical ASCII exceptions remain fixed at $($expectedNonAscii.Count) named files; no new exception is allowed."
