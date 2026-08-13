# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [switch]$WriteObservedState
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$statusPath = Join-Path $repo "docs/public-status.json"
$status = Get-Content -LiteralPath $statusPath -Raw | ConvertFrom-Json
$itemId = $status.chromeStore.itemId
if ($itemId -notmatch '^[a-p]{32}$') {
    throw "public-status.json has an invalid Chrome item id"
}

$headers = @{ "User-Agent" = "Ghostlight-chrome-store-reconciliation" }
$feedUri = "https://clients2.google.com/service/update2/crx?response=updatecheck&prodversion=150.0&acceptformat=crx2,crx3&x=id%3D$itemId%26uc"
$feed = (Invoke-WebRequest -Headers $headers -Uri $feedUri).Content
$match = [regex]::Match($feed, 'codebase="(?<codebase>[^"]+)"[^>]*version="(?<version>[0-9.]+)"')
if (-not $match.Success) {
    throw "Chrome update feed returned no public update for $itemId"
}
$observedVersion = $match.Groups["version"].Value
$codebase = $match.Groups["codebase"].Value

$compatibility = Get-Content -LiteralPath (Join-Path $repo "compatibility.json") -Raw | ConvertFrom-Json
$row = @($compatibility.chromeAdapters | Where-Object { $_.adapterVersion -eq $observedVersion })
if ($row.Count -ne 1) {
    throw "compatibility.json has no unique row for observed adapter $observedVersion"
}

Write-Output "Chrome item: $itemId"
Write-Output "Observed public adapter: $observedVersion"
Write-Output "Download endpoint: $codebase"
Write-Output "Recorded public adapter: $($status.chromeStore.publicAdapterVersion)"

if (-not $WriteObservedState) {
    if ($status.chromeStore.publicAdapterVersion -ne $observedVersion) {
        throw "recorded public adapter differs from the Chrome update feed"
    }
    return
}

$status.chromeStore.publicAdapterVersion = $observedVersion
$status.extensionSummary = "The Chrome Web Store serves Chrome adapter v$observedVersion. Compatibility is recorded in compatibility.json. Install the extension from the public listing."
$status.lastVerified = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd")
$json = $status | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText(
    $statusPath,
    $json + "`n",
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "Updated $statusPath from observed Chrome state"
