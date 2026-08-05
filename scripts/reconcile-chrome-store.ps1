#Requires -Version 7
<#
.SYNOPSIS
    Reconcile Ghostlight's canonical public status after a Chrome Web Store review completes.

.DESCRIPTION
    Reads the public adapter version from Chrome's update endpoint, validates its declared service
    compatibility, updates docs/public-status.json and the README extension-state paragraph, and
    removes a matching pending version. It never uploads, submits, or publishes an extension.

.PARAMETER ExpectedVersion
    Optional fail-closed assertion for the public version expected after review.

.PARAMETER PendingVersion
    Optional adapter version just accepted for review. It remains pending unless Chrome already
    serves it publicly.

.PARAMETER DryRun
    Report the observed transition without editing files.
#>
[CmdletBinding()]
param(
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $ExpectedVersion,
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $PendingVersion,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'adapter-compatibility.ps1')

function Format-WrappedParagraph([string] $Text, [int] $Width = 100) {
    $lines = [System.Collections.Generic.List[string]]::new()
    $line = ''
    foreach ($word in ($Text -split '\s+')) {
        if (-not $word) { continue }
        if (-not $line) {
            $line = $word
        }
        elseif (($line.Length + 1 + $word.Length) -le $Width) {
            $line += " $word"
        }
        else {
            $lines.Add($line)
            $line = $word
        }
    }
    if ($line) { $lines.Add($line) }
    return $lines -join [Environment]::NewLine
}

$statusPath = Join-Path $RepoRoot 'docs/public-status.json'
$readmePath = Join-Path $RepoRoot 'README.md'
$status = Get-Content -Raw -LiteralPath $statusPath | ConvertFrom-Json
$compatibility = Read-GhostlightAdapterCompatibility $RepoRoot
$cargo = Get-Content -Raw -LiteralPath (Join-Path $RepoRoot 'Cargo.toml')
$candidateServiceVersion = [regex]::Match($cargo, '(?m)^version = "([^"]+)"').Groups[1].Value
$itemId = [string] $status.chromeStore.itemId
$observed = Get-GhostlightChromeStorePublicVersion $itemId

if ($ExpectedVersion -and $observed -ne $ExpectedVersion) {
    throw "Chrome Web Store serves v$observed, not expected v$ExpectedVersion"
}

$pendingProperty = $status.chromeStore.PSObject.Properties['pendingAdapterVersion']
$pending = if ($pendingProperty) { [string] $pendingProperty.Value } else { '' }
if ($PSBoundParameters.ContainsKey('PendingVersion')) { $pending = $PendingVersion }
if ($pending -eq $observed) { $pending = '' }

$summary = Format-GhostlightExtensionSummary `
    $compatibility $status.release $observed $pending $candidateServiceVersion
$oldPublic = [string] $status.chromeStore.publicAdapterVersion

Write-Host "Chrome Web Store public adapter: v$observed"
Write-Host "Tracked public adapter:          v$oldPublic"
if ($pending) { Write-Host "Tracked pending adapter:         v$pending" }
Write-Host "Canonical summary: $summary"

if ($DryRun) {
    Write-Host '[dry] No files changed.'
    return
}

$status.chromeStore.publicAdapterVersion = $observed
if ($pending) {
    if ($status.chromeStore.PSObject.Properties['pendingAdapterVersion']) {
        $status.chromeStore.pendingAdapterVersion = $pending
    }
    else {
        $status.chromeStore | Add-Member -NotePropertyName pendingAdapterVersion -NotePropertyValue $pending
    }
}
elseif ($status.chromeStore.PSObject.Properties['pendingAdapterVersion']) {
    $status.chromeStore.PSObject.Properties.Remove('pendingAdapterVersion')
}
$status.extensionSummary = $summary
$status.lastVerified = (Get-Date).ToString('yyyy-MM-dd')
$statusJson = $status | ConvertTo-Json -Depth 6
Set-Content -LiteralPath $statusPath -Value $statusJson

$readme = Get-Content -Raw -LiteralPath $readmePath
$paragraph = Format-WrappedParagraph `
    "**Extension state.** $summary See the full [adapter compatibility map](compatibility.json)."
$pattern = '(?ms)^\*\*Extension state\.\*\*.*?(?=\r?\n\r?\n)'
if (-not [regex]::IsMatch($readme, $pattern)) {
    throw 'README extension-state paragraph was not found'
}
$readme = [regex]::Replace($readme, $pattern, $paragraph, 1)
Set-Content -LiteralPath $readmePath -Value $readme -NoNewline

& pwsh -File (Join-Path $PSScriptRoot 'check-public-surfaces.ps1')
if ($LASTEXITCODE -ne 0) { throw 'public surfaces are inconsistent after reconciliation' }

Write-Host 'Updated docs/public-status.json and README.md.'
Write-Host 'Review and commit the changes, then refresh the website with scripts/publish-website.ps1.'
