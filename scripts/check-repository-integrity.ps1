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
if ($tauri.version -ne $sourceVersion) {
    throw "Source and desktop versions differ"
}

$permissionDocument = Get-Content -LiteralPath (
    Join-Path $repo "docs/legal/PERMISSION_JUSTIFICATIONS.md"
) -Raw
$documentedPermissions = @(
    [regex]::Matches($permissionDocument, '(?m)^## (?<permission>[A-Za-z][A-Za-z0-9]*)\r?$') |
        ForEach-Object { $_.Groups["permission"].Value } |
        Sort-Object
)
$manifestPermissions = @($extension.permissions | Sort-Object)
if (($documentedPermissions -join "`n") -ne ($manifestPermissions -join "`n")) {
    throw "Extension manifest permissions and Chrome Web Store justifications differ"
}

# The plugin distribution member: twin manifests and both catalogs must agree (ADR-0144).
$pluginDir = Join-Path $repo "packaging/plugin/ghostlight"
$claudeManifest = Get-Content -LiteralPath (
    Join-Path $pluginDir ".claude-plugin/plugin.json"
) -Raw | ConvertFrom-Json
$zcodeManifest = Get-Content -LiteralPath (
    Join-Path $pluginDir ".zcode-plugin/plugin.json"
) -Raw | ConvertFrom-Json
if ($claudeManifest.name -ne $zcodeManifest.name -or $claudeManifest.name -ne "ghostlight") {
    throw "Plugin manifests disagree on the plugin name"
}
if ($claudeManifest.version -ne $zcodeManifest.version) {
    throw "Plugin manifests disagree on the plugin version"
}
$claudeSkills = if ($claudeManifest.PSObject.Properties["skills"]) {
    @($claudeManifest.skills)
}
else {
    @("skills")
}
$zcodeSkills = @($zcodeManifest.skills)
if ((($claudeSkills | Sort-Object) -join "`n") -ne (($zcodeSkills | Sort-Object) -join "`n")) {
    throw "Plugin manifests disagree on the skill set"
}
$claudeServer = $claudeManifest.mcpServers.ghostlight
$zcodeServer = $zcodeManifest.mcpServers.ghostlight
if (-not $claudeServer -or -not $zcodeServer) {
    throw "A plugin manifest is missing the ghostlight MCP server"
}
if ($claudeServer.command -ne $zcodeServer.command) {
    throw "Plugin manifests disagree on the MCP server command"
}
if (($claudeServer.args -join "`n") -ne ($zcodeServer.args -join "`n")) {
    throw "Plugin manifests disagree on the MCP server arguments"
}foreach ($catalogPath in @(".claude-plugin/marketplace.json", "marketplace.json")) {
    $catalog = Get-Content -LiteralPath (Join-Path $repo $catalogPath) -Raw | ConvertFrom-Json
    $entries = @($catalog.plugins | Where-Object { $_.name -eq "ghostlight" })
    if ($entries.Count -ne 1) {
        throw "Catalog $catalogPath does not list the ghostlight plugin exactly once"
    }
    if ($entries[0].version -ne $claudeManifest.version) {
        throw "Catalog $catalogPath version differs from the plugin manifests"
    }
    if ($entries[0].source -ne "./packaging/plugin/ghostlight") {
        throw "Catalog $catalogPath points at an unexpected plugin source"
    }
}

Write-Output "Repository integrity: $($tracked.Count) tracked files readable; local links valid; source version $sourceVersion aligned."
Write-Output "Historical ASCII exceptions remain fixed at $($expectedNonAscii.Count) named files; no new exception is allowed."
Write-Output "Every extension manifest permission has exactly one Chrome Web Store justification."
# The behavioral-parity matrix must stay closed and evidenced.
node (Join-Path $PSScriptRoot ".." "tests" "capability-matrix.mjs")

