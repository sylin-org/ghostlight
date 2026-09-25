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

$publicStatus = Read-JsonFile -Path (Join-Path $repo "docs/public-status.json")
$server = Read-JsonFile -Path (Join-Path $repo "server.json")
$manifest = Read-JsonFile -Path (Join-Path $repo "extension/manifest.json")
$tauri = Read-JsonFile -Path (Join-Path $repo "crates/orchestrator/tauri.conf.json")
$cargoText = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
$readme = Get-Content -LiteralPath (Join-Path $repo "README.md") -Raw
$llmsInstall = Get-Content -LiteralPath (Join-Path $repo "llms-install.md") -Raw
$installationGuide = Get-Content -LiteralPath (
    Join-Path $repo "docs/guides/installation.md"
) -Raw
$supplyChain = Get-Content -LiteralPath (Join-Path $repo "docs/trust/supply-chain.md") -Raw
$trustFaq = Get-Content -LiteralPath (Join-Path $repo "docs/trust/faq.md") -Raw
$distribution = Get-Content -LiteralPath (Join-Path $repo "docs/business/DISTRIBUTION.md") -Raw
$directorySubmissions = Get-Content -LiteralPath (
    Join-Path $repo "docs/business/DIRECTORY-SUBMISSIONS.md"
) -Raw
$scoop = Read-JsonFile -Path (Join-Path $repo "packaging/scoop/ghostlight.json")
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

$release = $publicStatus.release
$adapter = $publicStatus.chromeStore.publicAdapterVersion
Assert-Contains -Text $readme -Needle "published release is $release" -Label "README"
Assert-Contains -Text $readme -Needle "ghostlight@$release" -Label "README"
Assert-Contains -Text $readme -Needle "org.sylin/ghostlight $release" -Label "README"
Assert-Contains -Text $readme -Needle "Chrome Web Store adapter v$adapter" -Label "README"
Assert-Contains -Text $llmsInstall -Needle "ghostlight@$release install" -Label "LLM install guide"
Assert-Contains -Text $llmsInstall -Needle "ghostlight@$release doctor" -Label "LLM install guide"
Assert-Contains -Text $llmsInstall -Needle "public Chrome adapter is $adapter" -Label "LLM install guide"
Assert-Contains -Text $installationGuide -Needle "ghostlight@$release install" -Label "installation guide"
Assert-Contains -Text $installationGuide -Needle "ghostlight@$release doctor" -Label "installation guide"
Assert-Contains -Text $installationGuide -Needle "Browser`` $adapter extension" -Label "installation guide"
Assert-Contains -Text $supplyChain -Needle "currently serve $release" -Label "supply-chain trust page"
Assert-Contains -Text $supplyChain -Needle "adapter serves $adapter" -Label "supply-chain trust page"
Assert-Contains -Text $trustFaq -Needle "currently serve $release" -Label "trust FAQ"
Assert-Contains -Text $trustFaq -Needle "adapter serves $adapter" -Label "trust FAQ"
Assert-Contains -Text $distribution -Needle "ghostlight@$release" -Label "distribution runbook"
Assert-Contains -Text $distribution -Needle "v$release is active" -Label "distribution runbook"
Assert-Contains -Text $directorySubmissions -Needle "org.sylin/ghostlight`` $release" -Label "directory submissions"
Assert-Equal -Actual $scoop.version -Expected $release -Label "Scoop/public release"
Assert-Contains -Text $scoop.architecture.'64bit'.url -Needle "/v$release/" -Label "Scoop asset URL"
Assert-Contains -Text $scoop.architecture.'64bit'.extract_dir -Needle "v$release-" -Label "Scoop extract directory"
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
