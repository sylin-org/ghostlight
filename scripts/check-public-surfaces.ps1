# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [switch]$Online
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot

function Read-JsonFile {
    param([string]$Path)
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Assert-Equal {
    param($Actual, $Expected, [string]$Label)
    if ($Actual -ne $Expected) {
        throw "$Label mismatch: expected '$Expected', got '$Actual'"
    }
}

function Assert-Contains {
    param([string]$Text, [string]$Needle, [string]$Label)
    if (-not $Text.Contains($Needle, [System.StringComparison]::Ordinal)) {
        throw "$Label does not contain '$Needle'"
    }
}

function Assert-InstallPins {
    param([string]$Text, [string]$Label, [string]$Release, [string]$Adapter)
    $releasePins = [regex]::Matches($Text, 'ghostlight@(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?)\b')
    foreach ($match in $releasePins) {
        $found = $match.Groups[1].Value
        if ($found -ne $Release) {
            throw "$Label pins ghostlight@$found, expected $Release"
        }
    }
    $adapterPins = [regex]::Matches($Text, '(?:adapter v?|Ghostlight in Browser` )(\d+\.\d+\.\d+)\b')
    foreach ($match in $adapterPins) {
        $found = $match.Groups[1].Value
        if ($found -ne $Adapter) {
            throw "$Label names adapter $found, expected $Adapter"
        }
    }
}

$publicStatus = Read-JsonFile -Path (Join-Path $repo "docs/public-status.json")
$server = Read-JsonFile -Path (Join-Path $repo "server.json")
$manifest = Read-JsonFile -Path (Join-Path $repo "extension/manifest.json")
$tauri = Read-JsonFile -Path (Join-Path $repo "crates/orchestrator/tauri.conf.json")
$npmSource = Read-JsonFile -Path (Join-Path $repo "packaging/npm/package.json")
$mcpbSource = Read-JsonFile -Path (Join-Path $repo "packaging/mcpb/manifest.json")
$cargoText = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
$readme = Get-Content -LiteralPath (Join-Path $repo "README.md") -Raw
$llmsInstall = Get-Content -LiteralPath (Join-Path $repo "llms-install.md") -Raw
$installationGuide = Get-Content -LiteralPath (
    Join-Path $repo "docs/guides/installation.md"
) -Raw
$supplyChain = Get-Content -LiteralPath (Join-Path $repo "docs/trust/supply-chain.md") -Raw
$trustFaq = Get-Content -LiteralPath (Join-Path $repo "docs/trust/faq.md") -Raw
$wingetTemplate = Get-Content -LiteralPath (
    Join-Path $repo "packaging/winget/Sylin.Ghostlight.yaml"
) -Raw

$cargoMatch = [regex]::Match(
    $cargoText,
    '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"'
)
if (-not $cargoMatch.Success) {
    throw "Could not read workspace.package.version from Cargo.toml"
}
$sourceVersion = $cargoMatch.Groups["version"].Value

Assert-Equal -Actual $tauri.version -Expected $sourceVersion -Label "desktop/source version"
Assert-Equal -Actual $npmSource.version -Expected $sourceVersion -Label "npm launcher/source version"
Assert-Equal -Actual $mcpbSource.version -Expected $sourceVersion -Label "MCPB/source version"
Assert-Equal -Actual $server.version -Expected $publicStatus.release -Label "server.json/public release"
if (@($server.packages).Count -ne 1) {
    throw "server.json must declare exactly one public package"
}
Assert-Equal -Actual $server.packages[0].version -Expected $publicStatus.release -Label "registry package/public release"
Assert-Equal -Actual $server.name -Expected "org.sylin/ghostlight" -Label "official registry name"

$release = $publicStatus.release
$adapter = $publicStatus.chromeStore.publicAdapterVersion
Assert-InstallPins -Text $readme -Label "README" -Release $release -Adapter $adapter
Assert-InstallPins -Text $llmsInstall -Label "LLM install guide" -Release $release -Adapter $adapter
Assert-InstallPins -Text $installationGuide -Label "installation guide" -Release $release -Adapter $adapter
Assert-Contains -Text $supplyChain -Needle "currently serve $release" -Label "supply-chain trust page"
Assert-Contains -Text $supplyChain -Needle "adapter serves $adapter" -Label "supply-chain trust page"
Assert-Contains -Text $trustFaq -Needle "currently serve $release" -Label "trust FAQ"
Assert-Contains -Text $trustFaq -Needle "adapter serves $adapter" -Label "trust FAQ"
Assert-Contains -Text $wingetTemplate -Needle "/v__VERSION__/ghostlight-v__VERSION__-x86_64-pc-windows-msvc.zip" -Label "WinGet template archive URL"
if ($publicStatus.extensionSummary -notmatch ("\bv" + [regex]::Escape($adapter) + "\b")) {
    throw "extensionSummary does not name the observed public adapter v$adapter"
}

$compatibilityJson = & (Join-Path $PSScriptRoot "adapter-compatibility.ps1") -Json
$compatibility = $compatibilityJson | ConvertFrom-Json
if (-not $compatibility.source.compatible -or -not $compatibility.public.compatible) {
    throw "source or public compatibility is false"
}

Write-Output "Offline truth: source $sourceVersion; public $($publicStatus.release); public adapter $($publicStatus.chromeStore.publicAdapterVersion)"

if (-not $Online) {
    return
}

$headers = @{ "User-Agent" = "Ghostlight-public-surface-check" }
$githubHeaders = @{
    "User-Agent" = "Ghostlight-public-surface-check"
    "Accept" = "application/vnd.github+json"
}
$release = Invoke-RestMethod -Headers $githubHeaders -Uri "https://api.github.com/repos/sylin-org/ghostlight/releases/tags/v$($publicStatus.release)"
Assert-Equal -Actual $release.tag_name -Expected "v$($publicStatus.release)" -Label "GitHub release tag"
Assert-Equal -Actual $release.draft -Expected $false -Label "GitHub release draft state"
Assert-Equal -Actual $release.prerelease -Expected $false -Label "GitHub release prerelease state"
$latestRelease = Invoke-RestMethod -Headers $githubHeaders -Uri "https://api.github.com/repos/sylin-org/ghostlight/releases/latest"
Assert-Equal -Actual $latestRelease.tag_name -Expected "v$($publicStatus.release)" -Label "GitHub latest release"

$npm = Invoke-RestMethod -Headers $headers -Uri "https://registry.npmjs.org/ghostlight/$($publicStatus.release)"
Assert-Equal -Actual $npm.version -Expected $publicStatus.release -Label "npm version"
Assert-Equal -Actual $npm.mcpName -Expected $server.name -Label "npm MCP name"
$npmLatest = Invoke-RestMethod -Headers $headers -Uri "https://registry.npmjs.org/ghostlight/latest"
Assert-Equal -Actual $npmLatest.version -Expected $publicStatus.release -Label "npm latest dist-tag"

$itemId = $publicStatus.chromeStore.itemId
$feedUri = "https://clients2.google.com/service/update2/crx?response=updatecheck&prodversion=150.0&acceptformat=crx2,crx3&x=id%3D$itemId%26uc"
$feed = (Invoke-WebRequest -Headers $headers -Uri $feedUri).Content
$feedVersion = [regex]::Match(
    $feed,
    '<updatecheck\b[^>]*\bversion="(?<version>[0-9.]+)"'
).Groups["version"].Value
Assert-Equal -Actual $feedVersion -Expected $publicStatus.chromeStore.publicAdapterVersion -Label "Chrome update feed"

$registry = Invoke-RestMethod -Headers $headers -Uri "https://registry.modelcontextprotocol.io/v0.1/servers?search=org.sylin%2Fghostlight&version=latest"
$registryMatches = @($registry.servers | Where-Object {
        $_.server.name -eq $server.name -and $_.server.version -eq $publicStatus.release
    })
if ($registryMatches.Count -ne 1) {
    throw "official MCP Registry does not return exactly one latest $($server.name) $($publicStatus.release) record"
}

$canonicalPage = (Invoke-WebRequest -Headers $headers -Uri "https://sylin.org/ghostlight/").Content
if ($canonicalPage -notmatch [regex]::Escape($publicStatus.release)) {
    throw "canonical website does not contain observed public version $($publicStatus.release)"
}

Write-Output "Online truth: GitHub, npm, Chrome update feed, official MCP Registry, and website agree"
