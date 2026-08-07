#Requires -Version 7
<#
.SYNOPSIS
    Verify that Ghostlight's public release and platform claims agree.

.DESCRIPTION
    The tracked check is deterministic and network-free by default. It compares the canonical
    public status with every local release manifest and with the README copy that people see first.
    Pass -Online after a release or website deployment to check the live GitHub, npm, website,
    install-guide, and decision-aid surfaces as well.
#>
[CmdletBinding()]
param([switch] $Online)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'adapter-compatibility.ps1')
$StatusPath = Join-Path $RepoRoot 'docs/public-status.json'
$Status = Get-Content -Raw $StatusPath | ConvertFrom-Json
$Failures = [System.Collections.Generic.List[string]]::new()

function Add-Failure([string] $Message) { $Failures.Add($Message) }

function Assert-Equal([string] $Label, [string] $Expected, [string] $Actual) {
    if ($Actual -ne $Expected) {
        Add-Failure "$Label is '$Actual'; expected '$Expected'"
    }
}

function Assert-Contains([string] $Label, [string] $Text, [string] $Expected) {
    $NormalizedText = [regex]::Replace($Text, '\s+', ' ')
    $NormalizedExpected = [regex]::Replace($Expected, '\s+', ' ')
    if (-not $NormalizedText.Contains($NormalizedExpected, [StringComparison]::Ordinal)) {
        Add-Failure "$Label does not contain the canonical text: $Expected"
    }
}

function Assert-NotMatch([string] $Label, [string] $Text, [string] $Pattern) {
    if ($Text -match $Pattern) {
        Add-Failure "$Label contains retired installation guidance matching: $Pattern"
    }
}

function ConvertFrom-HtmlText([string] $Html) {
    $WithoutTags = [regex]::Replace($Html, '<[^>]+>', '')
    return [System.Net.WebUtility]::HtmlDecode($WithoutTags)
}

if ($Status.schemaVersion -ne 1) { Add-Failure "docs/public-status.json has unsupported schemaVersion '$($Status.schemaVersion)'" }
if ($Status.release -notmatch '^\d+\.\d+\.\d+$') { Add-Failure "public release '$($Status.release)' is not semantic x.y.z" }
if ([string]::IsNullOrWhiteSpace($Status.platformSummary)) { Add-Failure 'platformSummary is empty' }
if ([string]::IsNullOrWhiteSpace($Status.extensionSummary)) { Add-Failure 'extensionSummary is empty' }

$Cargo = Get-Content -Raw (Join-Path $RepoRoot 'Cargo.toml')
$CargoVersion = [regex]::Match($Cargo, '(?m)^version = "([^"]+)"').Groups[1].Value
[void](ConvertTo-GhostlightVersion $CargoVersion 'Cargo.toml version')

$Manifest = Get-Content -Raw (Join-Path $RepoRoot 'extension/manifest.json') | ConvertFrom-Json
$SourceAdapterVersion = $Manifest.version

try {
    $Compatibility = Read-GhostlightAdapterCompatibility $RepoRoot
    $SourceCoverage = Assert-GhostlightChromeAdapterCoversService `
        $Compatibility $SourceAdapterVersion $CargoVersion

    if (-not $Status.PSObject.Properties['chromeStore']) {
        throw 'docs/public-status.json has no chromeStore object'
    }
    $PublicAdapterVersion = $Status.chromeStore.publicAdapterVersion
    if ([string]::IsNullOrWhiteSpace($PublicAdapterVersion)) {
        throw 'docs/public-status.json has no public Chrome adapter version'
    }
    if ($Status.chromeStore.itemId -notmatch '^[a-p]{32}$') {
        throw 'docs/public-status.json has no valid Chrome Web Store item id'
    }

    $PendingProperty = $Status.chromeStore.PSObject.Properties['pendingAdapterVersion']
    $PendingAdapterVersion = if ($PendingProperty) { [string]$PendingProperty.Value } else { '' }
    $ExpectedExtensionSummary = Format-GhostlightExtensionSummary `
        $Compatibility $Status.release $PublicAdapterVersion $PendingAdapterVersion $CargoVersion
    Assert-Equal 'canonical extension summary' $ExpectedExtensionSummary $Status.extensionSummary
}
catch {
    Add-Failure "Chrome adapter compatibility: $($_.Exception.Message)"
}

$Npm = Get-Content -Raw (Join-Path $RepoRoot 'packaging/npm/package.json') | ConvertFrom-Json
Assert-Equal 'npm package version' $CargoVersion $Npm.version

$Server = Get-Content -Raw (Join-Path $RepoRoot 'server.json') | ConvertFrom-Json
Assert-Equal 'MCP server version' $CargoVersion $Server.version
Assert-Equal 'MCP package version' $CargoVersion $Server.packages[0].version

$Readme = Get-Content -Raw (Join-Path $RepoRoot 'README.md')
Assert-Contains 'README platform state' $Readme $Status.platformSummary
Assert-Contains 'README extension state' $Readme $Status.extensionSummary
Assert-Contains 'README decision path' $Readme 'https://sylin.org/ghostlight/decision-aid/'

$RetiredInstallPattern = '(?i)manual (?:extension )?install|manual installation|install by hand|release-extension|ghostlight-extension-v[^\s`]*\.zip|get the extension archive|read the manual path'
Assert-NotMatch 'README install path' $Readme $RetiredInstallPattern

$AgentInstall = Get-Content -Raw (Join-Path $RepoRoot 'llms-install.md')
$SoloInstall = Get-Content -Raw (Join-Path $RepoRoot 'docs/guides/solo-developer.md')
$ReleaseTemplate = Get-Content -Raw (Join-Path $RepoRoot '.github/release-template.md')
$PublicStatusText = Get-Content -Raw $StatusPath
foreach ($Surface in @(
    @{ Label = 'agent install guide'; Text = $AgentInstall },
    @{ Label = 'solo-developer guide'; Text = $SoloInstall },
    @{ Label = 'public status'; Text = $PublicStatusText }
)) {
    Assert-NotMatch $Surface.Label $Surface.Text $RetiredInstallPattern
    Assert-NotMatch $Surface.Label $Surface.Text '(?i)load unpacked|chrome://extensions|sideload|side-load'
}
Assert-NotMatch 'release notes template' $ReleaseTemplate `
    '(?i)manual (?:extension )?install|manual installation|install by hand|load unpacked|chrome://extensions|sideload|side-load'
Assert-Contains 'release notes template changelog slot' $ReleaseTemplate '${CHANGELOG}'

$SourceGuide = Get-Content -Raw (Join-Path $RepoRoot 'docs/guides/installation.md')
Assert-Contains 'README source-development exception' $Readme 'Build from source and test locally'
Assert-Contains 'installation guide source-development exception' $SourceGuide 'To test the source tree immediately'

if ($Online) {
    $Headers = @{ 'User-Agent' = 'ghostlight-public-surface-check'; Accept = 'application/vnd.github+json' }
    $Release = Invoke-RestMethod -Headers $Headers -Uri 'https://api.github.com/repos/sylin-org/ghostlight/releases/latest'
    Assert-Equal 'latest GitHub release' "v$($Status.release)" $Release.tag_name

    $Registry = Invoke-RestMethod -Uri 'https://registry.npmjs.org/ghostlight/latest'
    Assert-Equal 'latest npm release' $Status.release $Registry.version

    try {
        $LiveChromeAdapterVersion = Get-GhostlightChromeStorePublicVersion $Status.chromeStore.itemId
        Assert-Equal 'live Chrome Web Store adapter' `
            $Status.chromeStore.publicAdapterVersion $LiveChromeAdapterVersion
    }
    catch {
        Add-Failure "live Chrome Web Store adapter check failed: $($_.Exception.Message)"
    }

    $Website = (Invoke-WebRequest -UseBasicParsing -Uri 'https://sylin.org/ghostlight/').Content
    $WebsiteText = ConvertFrom-HtmlText $Website
    Assert-Contains 'live Ghostlight page version fallback' $WebsiteText "v$($Status.release)"
    Assert-Contains 'live Ghostlight platform state' $WebsiteText $Status.platformSummary
    Assert-Contains 'live Ghostlight extension state' $WebsiteText $Status.extensionSummary

    foreach ($Uri in @(
        'https://sylin.org/ghostlight/install.md',
        'https://sylin.org/ghostlight/decision-aid/',
        'https://sylin.org/ghostlight/privacy/'
    )) {
        $Response = Invoke-WebRequest -UseBasicParsing -Uri $Uri
        if ($Response.StatusCode -ne 200) { Add-Failure "$Uri returned HTTP $($Response.StatusCode)" }
    }
}

if ($Failures.Count -gt 0) {
    Write-Error ("Public-surface check failed:`n- " + ($Failures -join "`n- "))
    exit 1
}

$Mode = if ($Online) { 'local and live' } else { 'local' }
Write-Host "Public-surface check passed ($Mode): public service v$($Status.release), source service v$CargoVersion, source Chrome adapter v$SourceAdapterVersion, platform, extension, and entry-path claims agree."
