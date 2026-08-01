#Requires -Version 7
<#
.SYNOPSIS
    Publish the Ghostlight browser extension to the Chrome Web Store and Microsoft Edge Add-ons:
    automatically when store API credentials are present, otherwise print exact submission steps.

.DESCRIPTION
    "Automate if possible, else dump the instructions." This script builds (or reuses) the
    store-ready zip -- the SAME artifact scripts/package-extension.ps1 produces, with the manifest
    `key` stripped and dev-only files excluded -- and then, per store:

      Chrome Web Store: if CWS_CLIENT_ID, CWS_CLIENT_SECRET, CWS_REFRESH_TOKEN, and CWS_ITEM_ID are
      all set, it uploads the zip and publishes via the Chrome Web Store API v1.1 (OAuth2 refresh
      token -> access token -> upload -> publish). Otherwise it prints the manual dashboard steps.

      Edge Add-ons: if EDGE_PRODUCT_ID, EDGE_CLIENT_ID, and EDGE_API_KEY are all set, it uploads the
      zip and publishes via the Edge Add-ons API v1.1 (upload package -> poll -> publish -> poll).
      Otherwise it prints the manual Partner Center steps.

    The API paths are IDEMPOTENT-ish by nature of the stores (re-publishing the same version is a
    no-op-or-error the store reports); this script surfaces the store's own status verbatim. It never
    invents success -- an upload that the store reports as FAILURE throws.

    One-time credential setup is documented in docs/RELEASE.md ("Extension stores"). Store the
    secrets in your shell/session env or a secret manager, NEVER in the repo.

.PARAMETER Version
    The expected Chrome adapter version (without a leading v). When supplied, it must match
    extension/manifest.json. The manifest remains the source of truth.

.PARAMETER Zip
    Path to an already-built store zip. Defaults to dist/ghostlight-extension-v<Version>.zip, which
    this script builds via package-extension.ps1 if it is absent.

.PARAMETER Target
    Chrome Web Store publish target: 'default' (public) or 'trustedTesters'. Default: 'default'.

.PARAMETER SkipChrome
    Do not touch the Chrome Web Store.

.PARAMETER SkipEdge
    Do not touch Edge Add-ons.

.PARAMETER DryRun
    Build/verify the zip and report exactly what each store step WOULD do (including which
    credentials are present), but make no API calls that mutate a store listing.

.EXAMPLE
    pwsh -File scripts/publish-extension.ps1
        Build the zip; auto-publish to any store whose credentials are set, else print steps.

.EXAMPLE
    pwsh -File scripts/publish-extension.ps1 -DryRun
        Show the whole plan and which store credentials are configured, without publishing.

.EXAMPLE
    pwsh -File scripts/publish-extension.ps1 -Target trustedTesters -SkipEdge
        Publish to the Chrome Web Store's trusted-tester track only.
#>
[CmdletBinding()]
param(
    [string] $Version,
    [string] $Zip,
    [ValidateSet('default', 'trustedTesters')]
    [string] $Target = 'default',
    [switch] $SkipChrome,
    [switch] $SkipEdge,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

# --- Output helpers (match release.ps1's vocabulary) -----------------------------------

function Write-Banner([string] $Text) { Write-Host ''; Write-Host "=== $Text ===" -ForegroundColor Cyan }
function Write-Ok([string] $Text) { Write-Host "  [ok]   $Text" -ForegroundColor Green }
function Write-Skip([string] $Text) { Write-Host "  [skip] $Text" -ForegroundColor DarkGray }
function Write-Info([string] $Text) { Write-Host "  [info] $Text" }
function Write-Warn2([string] $Text) { Write-Host "  [warn] $Text" -ForegroundColor Yellow }
function Write-Would([string] $Text) { Write-Host "  [dry]  would: $Text" -ForegroundColor Magenta }
function Write-Steps([string] $Text) { Write-Host $Text -ForegroundColor Yellow }

# --- Version + zip ---------------------------------------------------------------------

function Resolve-Version {
    $manifest = Get-Content (Join-Path $RepoRoot 'extension/manifest.json') -Raw | ConvertFrom-Json
    if (-not $manifest.version) { throw 'no version in extension/manifest.json' }
    if ($Version -and $Version -ne $manifest.version) {
        throw "requested Chrome adapter version $Version does not match manifest version $($manifest.version)"
    }
    return $manifest.version
}

function Resolve-Zip([string] $Ver) {
    if ($Zip) {
        if (-not (Test-Path $Zip)) { throw "zip not found: $Zip" }
        return (Resolve-Path $Zip).Path
    }
    $default = Join-Path $RepoRoot "dist/ghostlight-extension-v$Ver.zip"
    if (-not (Test-Path $default)) {
        Write-Info "store zip not found; building it via package-extension.ps1"
        & (Join-Path $PSScriptRoot 'package-extension.ps1') -Version $Ver | Out-Host
    }
    if (-not (Test-Path $default)) { throw "expected zip was not produced: $default" }
    return (Resolve-Path $default).Path
}

# --- Chrome Web Store (API v1.1) -------------------------------------------------------

function Get-ChromeCreds {
    [ordered]@{
        ClientId     = $env:CWS_CLIENT_ID
        ClientSecret = $env:CWS_CLIENT_SECRET
        RefreshToken = $env:CWS_REFRESH_TOKEN
        ItemId       = $env:CWS_ITEM_ID
    }
}

function Test-AllSet([hashtable] $Creds) {
    return -not ($Creds.Values | Where-Object { [string]::IsNullOrWhiteSpace($_) })
}

function Get-ChromeAccessToken([hashtable] $Creds) {
    # OAuth2 refresh-token grant -> short-lived access token.
    $body = @{
        client_id     = $Creds.ClientId
        client_secret = $Creds.ClientSecret
        refresh_token = $Creds.RefreshToken
        grant_type    = 'refresh_token'
    }
    $resp = Invoke-RestMethod -Method Post -Uri 'https://oauth2.googleapis.com/token' -Body $body
    if (-not $resp.access_token) { throw 'Chrome Web Store: token endpoint returned no access_token' }
    return $resp.access_token
}

function Publish-Chrome([string] $ZipPath, [string] $Ver) {
    Write-Banner 'Chrome Web Store'
    if ($SkipChrome) { Write-Skip '-SkipChrome set'; return }

    $creds = Get-ChromeCreds
    if (-not (Test-AllSet $creds)) {
        Write-Warn2 'Chrome Web Store credentials not fully set -- printing manual steps instead.'
        Write-ChromeInstructions $ZipPath $Ver
        return
    }
    Write-Ok "credentials present (item $($creds.ItemId))"

    if ($DryRun) {
        Write-Would "exchange refresh token for an access token"
        Write-Would "PUT the zip to items/$($creds.ItemId) (upload)"
        Write-Would "POST items/$($creds.ItemId)/publish?publishTarget=$Target"
        return
    }

    $token = Get-ChromeAccessToken $creds
    $headers = @{ Authorization = "Bearer $token"; 'x-goog-api-version' = '2' }

    # Upload the package.
    Write-Info "uploading $([System.IO.Path]::GetFileName($ZipPath))"
    $uploadUri = "https://www.googleapis.com/upload/chromewebstore/v1.1/items/$($creds.ItemId)?uploadType=media"
    $up = Invoke-RestMethod -Method Put -Uri $uploadUri -Headers $headers -InFile $ZipPath -ContentType 'application/zip'
    if ($up.uploadState -eq 'FAILURE') {
        $detail = ($up.itemError | ForEach-Object { $_.error_detail }) -join '; '
        throw "Chrome Web Store upload FAILED: $detail"
    }
    Write-Ok "upload state: $($up.uploadState)"

    # Publish.
    Write-Info "publishing (target: $Target)"
    $pubUri = "https://www.googleapis.com/chromewebstore/v1.1/items/$($creds.ItemId)/publish?publishTarget=$Target"
    $pub = Invoke-RestMethod -Method Post -Uri $pubUri -Headers $headers -ContentType 'application/json'
    $statuses = @($pub.status) -join ', '
    # A brand-new listing's first publish can return ITEM_PENDING_REVIEW; that is success (queued).
    if ($pub.status -contains 'OK' -or $pub.status -contains 'ITEM_PENDING_REVIEW') {
        Write-Ok "publish accepted: [$statuses] $((@($pub.statusDetail)) -join '; ')"
    }
    else {
        throw "Chrome Web Store publish returned [$statuses]: $((@($pub.statusDetail)) -join '; ')"
    }
    Write-Info 'the store re-reviews the new version; live rollout follows review (usually hours to a few days).'
}

function Write-ChromeInstructions([string] $ZipPath, [string] $Ver) {
    Write-Steps @"
  Manual Chrome Web Store submission (v$Ver):
    1. Open the Developer Dashboard: https://chrome.google.com/webstore/devconsole
    2. Select the "Ghostlight in Browser" item.
    3. Package tab -> "Upload new package" -> choose:
         $ZipPath
    4. Fill any changed store-listing fields from docs/legal/STORE_LISTING.md,
       docs/legal/PRIVACY.md, and docs/legal/PERMISSION_JUSTIFICATIONS.md.
    5. "Submit for review". Publishing is gated on Google's review.

  To automate this next time, set these env vars and re-run this script (see docs/RELEASE.md):
    CWS_CLIENT_ID, CWS_CLIENT_SECRET, CWS_REFRESH_TOKEN, CWS_ITEM_ID
"@
}

# --- Edge Add-ons (API v1.1) -----------------------------------------------------------

function Get-EdgeCreds {
    [ordered]@{
        ProductId = $env:EDGE_PRODUCT_ID
        ClientId  = $env:EDGE_CLIENT_ID
        ApiKey    = $env:EDGE_API_KEY
    }
}

function Wait-EdgeOperation([string] $Uri, [hashtable] $Headers, [string] $What) {
    # Edge returns 202 + an operation to poll; loop until it succeeds or fails (bounded).
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Seconds 5
        $op = Invoke-RestMethod -Method Get -Uri $Uri -Headers $Headers
        switch ($op.status) {
            'Succeeded' { Write-Ok "$What succeeded"; return $op }
            'Failed' { throw "$What failed: $($op.message) $((@($op.errors)) -join '; ')" }
            default { Write-Info "$What status: $($op.status) ($($i + 1)/60)" }
        }
    }
    throw "$What did not complete within ~5 min"
}

function Publish-Edge([string] $ZipPath, [string] $Ver) {
    Write-Banner 'Microsoft Edge Add-ons'
    if ($SkipEdge) { Write-Skip '-SkipEdge set'; return }

    $creds = Get-EdgeCreds
    if (-not (Test-AllSet $creds)) {
        Write-Warn2 'Edge Add-ons credentials not fully set -- printing manual steps instead.'
        Write-EdgeInstructions $ZipPath $Ver
        return
    }
    Write-Ok "credentials present (product $($creds.ProductId))"

    $base = "https://api.addons.microsoftedge.microsoft.com/v1/products/$($creds.ProductId)/submissions"
    $headers = @{ Authorization = "ApiKey $($creds.ApiKey)"; 'X-ClientID' = $creds.ClientId }

    if ($DryRun) {
        Write-Would "POST $base/draft/package (upload the zip)"
        Write-Would "poll the upload operation until Succeeded"
        Write-Would "POST $base (publish the draft)"
        Write-Would "poll the publish operation until Succeeded"
        return
    }

    # Upload the package into the draft submission.
    Write-Info "uploading $([System.IO.Path]::GetFileName($ZipPath))"
    $uploadResp = Invoke-WebRequest -Method Post -Uri "$base/draft/package" -Headers $headers `
        -InFile $ZipPath -ContentType 'application/zip'
    $opId = $uploadResp.Headers['Location']
    if (-not $opId) { throw 'Edge: upload returned no operation id (Location header)' }
    Wait-EdgeOperation "$base/draft/package/operations/$opId" $headers 'Edge upload' | Out-Null

    # Publish the draft.
    Write-Info 'publishing the draft submission'
    $pubResp = Invoke-WebRequest -Method Post -Uri $base -Headers $headers `
        -Body '{"notes":"Automated release publish."}' -ContentType 'application/json'
    $pubOp = $pubResp.Headers['Location']
    if (-not $pubOp) { throw 'Edge: publish returned no operation id (Location header)' }
    Wait-EdgeOperation "$base/operations/$pubOp" $headers 'Edge publish' | Out-Null
    Write-Ok 'Edge submission published (queued for certification)'
}

function Write-EdgeInstructions([string] $ZipPath, [string] $Ver) {
    Write-Steps @"
  Manual Edge Add-ons submission (v$Ver):
    1. Open Partner Center: https://partner.microsoft.com/dashboard/microsoftedge/overview
    2. Select the Ghostlight extension (or "Create new extension" for a first submission).
    3. Packages -> upload:
         $ZipPath
    4. Review the listing fields (reuse the Chrome listing copy).
    5. "Publish". Publishing is gated on Microsoft's certification.

  To automate this next time, set these env vars and re-run this script (see docs/RELEASE.md):
    EDGE_PRODUCT_ID, EDGE_CLIENT_ID, EDGE_API_KEY
"@
}

# =======================================================================================
# DRIVER
# =======================================================================================

# Dot-sourcing loads the functions without running (for unit tests / release.ps1 reuse).
if ($MyInvocation.InvocationName -eq '.') { return }

$ver = Resolve-Version
$zipPath = Resolve-Zip $ver

Write-Host "Ghostlight extension publish -- v$ver" -ForegroundColor White
Write-Host "  zip : $zipPath"
Write-Host "  mode: $(if ($DryRun) { 'DRY RUN (no store mutations)' } else { 'LIVE' })"

Publish-Chrome $zipPath $ver
Publish-Edge $zipPath $ver

Write-Host ''
Write-Host 'Extension publish step complete.' -ForegroundColor Green
