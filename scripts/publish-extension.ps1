#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [Parameter(Mandatory = $true)]
    [string]$ZipPath,
    [string]$CredentialFile = (Join-Path $HOME ".ghostlight-release.env"),
    [ValidateSet("Plan", "Upload", "Submit")]
    [string]$Action = "Plan",
    [ValidateSet("STAGED_PUBLISH", "DEFAULT_PUBLISH")]
    [string]$PublishType = "STAGED_PUBLISH",
    [switch]$Execute
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$ZipPath = [System.IO.Path]::GetFullPath($ZipPath)
if (-not (Test-Path -LiteralPath $ZipPath -PathType Leaf)) {
    throw "Extension ZIP does not exist: $ZipPath"
}
if ($Action -ne "Plan" -and -not $Execute) {
    throw "$Action changes Chrome Web Store state. Pass -Execute only after owner approval for that exact action."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
try {
    $manifestEntry = $archive.GetEntry("manifest.json")
    if ($null -eq $manifestEntry) {
        throw "Extension ZIP has no root manifest.json"
    }
    $reader = [System.IO.StreamReader]::new($manifestEntry.Open())
    try {
        $manifest = $reader.ReadToEnd() | ConvertFrom-Json
    }
    finally {
        $reader.Dispose()
    }
}
finally {
    $archive.Dispose()
}
if ($manifest.version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:\.[0-9]+)?$') {
    throw "Extension manifest version is invalid"
}
$sourceManifest = Get-Content -LiteralPath (Join-Path $repo "extension/manifest.json") -Raw |
    ConvertFrom-Json
if ($manifest.version -ne $sourceManifest.version) {
    throw "Extension ZIP version $($manifest.version) differs from source $($sourceManifest.version)"
}

$hash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$values = @{}
if (Test-Path -LiteralPath $CredentialFile -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $CredentialFile) {
        if ($line -match '^([A-Z0-9_]+)=(.*)$') {
            $values[$Matches[1]] = $Matches[2]
        }
    }
}
$required = @(
    "CWS_CLIENT_ID",
    "CWS_CLIENT_SECRET",
    "CWS_REFRESH_TOKEN",
    "CWS_ITEM_ID",
    "CWS_PUBLISHER_ID"
)
$missing = @($required | Where-Object {
    -not $values.ContainsKey($_) -or [string]::IsNullOrWhiteSpace($values[$_])
})
$manualInstructions = @"
Manual Chrome Web Store submission:
  1. Open https://chrome.google.com/webstore/devconsole and select Ghostlight in Browser.
  2. On Package, upload: $ZipPath
  3. Review changed listing fields against docs/legal/STORE_LISTING.md,
     docs/legal/PRIVACY.md, and docs/legal/PERMISSION_JUSTIFICATIONS.md.
  4. Submit for review with deferred publication. Google controls review timing.
  5. Keep an approved build staged until the owner authorizes public publication.
  6. After the public update is observable, run:
       pwsh -File scripts/reconcile-chrome-store.ps1 -WriteObservedState
"@

Write-Output "Chrome Web Store action: $Action"
Write-Output "Extension version: $($manifest.version)"
Write-Output "Extension SHA-256: $hash"
Write-Output "Publish type: $PublishType"
Write-Output "API automation: $(if ($missing.Count -eq 0) { 'ready' } else { 'optional fields not configured: ' + ($missing -join ', ') })"
if ($Action -eq "Plan") {
    Write-Output $manualInstructions
    Write-Output "No Chrome Web Store request was made."
    return
}
if ($missing.Count -gt 0) {
    Write-Output "Chrome API automation is unavailable: $($missing -join ', ')"
    Write-Output $manualInstructions
    Write-Output "No Chrome Web Store request was made."
    return
}
if ($values.CWS_ITEM_ID -notmatch '^[a-p]{32}$' -or
    $values.CWS_PUBLISHER_ID -notmatch '^[A-Za-z0-9._-]+$') {
    throw "Chrome item or publisher identifier has an invalid shape"
}

$token = Invoke-RestMethod `
    -Method Post `
    -Uri "https://oauth2.googleapis.com/token" `
    -ContentType "application/x-www-form-urlencoded" `
    -Body @{
        client_id = $values.CWS_CLIENT_ID
        client_secret = $values.CWS_CLIENT_SECRET
        refresh_token = $values.CWS_REFRESH_TOKEN
        grant_type = "refresh_token"
    }
if ([string]::IsNullOrWhiteSpace($token.access_token)) {
    throw "Google returned no access token"
}

$publisherId = [uri]::EscapeDataString($values.CWS_PUBLISHER_ID)
$itemId = [uri]::EscapeDataString($values.CWS_ITEM_ID)
$resource = "publishers/$publisherId/items/$itemId"
$headers = @{ Authorization = "Bearer $($token.access_token)" }

if ($Action -eq "Upload") {
    $response = Invoke-WebRequest `
        -SkipHttpErrorCheck `
        -Method Post `
        -Uri "https://chromewebstore.googleapis.com/upload/v2/${resource}:upload" `
        -Headers $headers `
        -ContentType "application/zip" `
        -InFile $ZipPath
    if ($response.StatusCode -lt 200 -or $response.StatusCode -ge 300) {
        throw "Chrome package upload failed with HTTP $($response.StatusCode)"
    }
    $upload = $response.Content | ConvertFrom-Json
    Write-Output "Chrome upload state: $($upload.uploadState)"
    Write-Output "Chrome draft version: $($upload.crxVersion)"
    return
}

$statusResponse = Invoke-WebRequest `
    -SkipHttpErrorCheck `
    -Method Get `
    -Uri "https://chromewebstore.googleapis.com/v2/${resource}:fetchStatus" `
    -Headers $headers
if ($statusResponse.StatusCode -ne 200) {
    throw "Chrome status check failed with HTTP $($statusResponse.StatusCode)"
}
$status = $statusResponse.Content | ConvertFrom-Json
if ($status.takenDown -or $status.warned) {
    throw "Chrome item is taken down or warned; inspect the dashboard before submission"
}

$publishBody = [ordered]@{
    publishType = $PublishType
    skipReview = $false
    blockOnWarnings = $true
} | ConvertTo-Json -Compress
$publishResponse = Invoke-WebRequest `
    -SkipHttpErrorCheck `
    -Method Post `
    -Uri "https://chromewebstore.googleapis.com/v2/${resource}:publish" `
    -Headers $headers `
    -ContentType "application/json" `
    -Body $publishBody
if ($publishResponse.StatusCode -lt 200 -or $publishResponse.StatusCode -ge 300) {
    throw "Chrome submission failed with HTTP $($publishResponse.StatusCode)"
}
$published = $publishResponse.Content | ConvertFrom-Json
Write-Output "Chrome submission state: $($published.state)"
Write-Output "Chrome publish type: $PublishType"
