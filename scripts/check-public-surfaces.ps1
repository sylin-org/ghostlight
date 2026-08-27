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

$publicStatus = Read-JsonFile -Path (Join-Path $repo "docs/public-status.json")
$server = Read-JsonFile -Path (Join-Path $repo "server.json")
$manifest = Read-JsonFile -Path (Join-Path $repo "extension/manifest.json")
$tauri = Read-JsonFile -Path (Join-Path $repo "crates/orchestrator/tauri.conf.json")
$cargoText = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
$readme = Get-Content -LiteralPath (Join-Path $repo "README.md") -Raw
$siteFiles = Get-ChildItem -LiteralPath (Join-Path $repo "site") -File -Recurse
$siteText = ($siteFiles | ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }) -join "`n"

$cargoMatch = [regex]::Match(
    $cargoText,
    '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(?<version>[^"]+)"'
)
if (-not $cargoMatch.Success) {
    throw "Could not read workspace.package.version from Cargo.toml"
}
$sourceVersion = $cargoMatch.Groups["version"].Value

Assert-Equal -Actual $tauri.version -Expected $sourceVersion -Label "desktop/source version"
Assert-Equal -Actual $server.version -Expected $publicStatus.release -Label "server.json/public release"
if (@($server.packages).Count -ne 1) {
    throw "server.json must declare exactly one public package"
}
Assert-Equal -Actual $server.packages[0].version -Expected $publicStatus.release -Label "registry package/public release"
Assert-Equal -Actual $server.name -Expected "org.sylin/ghostlight" -Label "official registry name"

$publicMinor = ([version]$publicStatus.release).ToString(2)
if ($readme -notmatch [regex]::Escape("published release is $publicMinor")) {
    throw "README does not identify the observed public $publicMinor release"
}
if ($siteText -notmatch [regex]::Escape("https://sylin.org/ghostlight/")) {
    throw "site sources do not point at the canonical Ghostlight page"
}
if ($publicStatus.extensionSummary -notmatch [regex]::Escape($publicStatus.chromeStore.publicAdapterVersion)) {
    throw "extensionSummary does not name the observed public adapter"
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

$npm = Invoke-RestMethod -Headers $headers -Uri "https://registry.npmjs.org/ghostlight/$($publicStatus.release)"
Assert-Equal -Actual $npm.version -Expected $publicStatus.release -Label "npm version"

$itemId = $publicStatus.chromeStore.itemId
$feedUri = "https://clients2.google.com/service/update2/crx?response=updatecheck&prodversion=150.0&acceptformat=crx2,crx3&x=id%3D$itemId%26uc"
$feed = (Invoke-WebRequest -Headers $headers -Uri $feedUri).Content
$feedVersion = [regex]::Match(
    $feed,
    '<updatecheck\b[^>]*\bversion="(?<version>[0-9.]+)"'
).Groups["version"].Value
Assert-Equal -Actual $feedVersion -Expected $publicStatus.chromeStore.publicAdapterVersion -Label "Chrome update feed"

$registry = Invoke-RestMethod -Headers $headers -Uri "https://registry.modelcontextprotocol.io/v0.1/servers?search=org.sylin%2Fghostlight&version=latest"
$registryVersions = @($registry.servers | ForEach-Object { $_.server.version })
if ($registryVersions -notcontains $publicStatus.release) {
    throw "official MCP Registry does not return public version $($publicStatus.release)"
}

$canonicalPage = (Invoke-WebRequest -Headers $headers -Uri "https://sylin.org/ghostlight/").Content
if ($canonicalPage -notmatch [regex]::Escape($publicStatus.release)) {
    throw "canonical website does not contain observed public version $($publicStatus.release)"
}

Write-Output "Online truth: GitHub, npm, Chrome update feed, official MCP Registry, and website agree"
