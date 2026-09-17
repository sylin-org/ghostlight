#Requires -Version 7
# SPDX-License-Identifier: Apache-2.0 OR MIT

param(
    [string]$ZipPath,
    [string]$CredentialFile = (Join-Path $PSScriptRoot "../local/.ghostlight-release.env"),
    [string]$ApiKey,
    [string]$ApiSecret,
    [ValidateSet("Plan", "SignUnlisted", "SubmitListed", "Status")]
    [string]$Action = "Plan",
    [string]$ArtifactsDir,
    [switch]$Execute
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$source = Join-Path $repo "extension-firefox"
$manifestPath = Join-Path $source "manifest.json"
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Firefox extension manifest not found: $manifestPath"
}
$sourceManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$version = $sourceManifest.version

if ([string]::IsNullOrWhiteSpace($ArtifactsDir)) {
    $ArtifactsDir = Join-Path $repo "dist"
}
$ArtifactsDir = [System.IO.Path]::GetFullPath($ArtifactsDir)
if (-not (Test-Path -LiteralPath $ArtifactsDir)) {
    New-Item -ItemType Directory -Path $ArtifactsDir | Out-Null
}

if ([string]::IsNullOrWhiteSpace($ZipPath)) {
    $ZipPath = Join-Path $ArtifactsDir "ghostlight-firefox-extension-v$version.zip"
}
$ZipPath = [System.IO.Path]::GetFullPath($ZipPath)

# Ensure package is present and up to date
if (-not (Test-Path -LiteralPath $ZipPath -PathType Leaf) -or $Action -ne "Status") {
    Write-Output "Packaging Firefox extension for version $version..."
    & (Join-Path $PSScriptRoot "package-extension-firefox.ps1") -OutputPath $ZipPath -Force
}

# Verify package manifest
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
try {
    $entry = $archive.GetEntry("manifest.json")
    if ($null -eq $entry) {
        throw "Extension ZIP has no root manifest.json: $ZipPath"
    }
    $reader = [System.IO.StreamReader]::new($entry.Open())
    try {
        $packagedManifest = $reader.ReadToEnd() | ConvertFrom-Json
    }
    finally {
        $reader.Dispose()
    }
}
finally {
    $archive.Dispose()
}

if ($packagedManifest.version -ne $version) {
    throw "Packaged version $($packagedManifest.version) does not match source version $version"
}

$extensionId = $packagedManifest.browser_specific_settings.gecko.id
if ([string]::IsNullOrWhiteSpace($extensionId)) {
    throw "Firefox extension manifest must define browser_specific_settings.gecko.id"
}

$hash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$size = (Get-Item -LiteralPath $ZipPath).Length

# Resolve credentials
$values = @{}
if (Test-Path -LiteralPath $CredentialFile -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $CredentialFile) {
        if ($line -match '^([A-Z0-9_]+)=(.*)$') {
            $values[$Matches[1]] = $Matches[2]
        }
    }
}
if (-not [string]::IsNullOrWhiteSpace($env:AMO_JWT_ISSUER)) {
    $values["AMO_JWT_ISSUER"] = $env:AMO_JWT_ISSUER
}
if (-not [string]::IsNullOrWhiteSpace($env:AMO_JWT_SECRET)) {
    $values["AMO_JWT_SECRET"] = $env:AMO_JWT_SECRET
}
if (-not [string]::IsNullOrWhiteSpace($ApiKey)) {
    $values["AMO_JWT_ISSUER"] = $ApiKey
}
if (-not [string]::IsNullOrWhiteSpace($ApiSecret)) {
    $values["AMO_JWT_SECRET"] = $ApiSecret
}

$hasCredentials = (-not [string]::IsNullOrWhiteSpace($values["AMO_JWT_ISSUER"])) -and
    (-not [string]::IsNullOrWhiteSpace($values["AMO_JWT_SECRET"]))

Write-Output "Firefox Extension Publisher"
Write-Output "Extension ID:      $extensionId"
Write-Output "Extension Version: $version"
Write-Output "Package Path:      $ZipPath"
Write-Output "Package Size:      $size bytes"
Write-Output "Package SHA-256:   $hash"
Write-Output "Target Action:     $Action"
Write-Output "AMO Credentials:   $(if ($hasCredentials) { 'configured' } else { 'not configured' })"

# Run web-ext lint preflight check
Write-Output "Running web-ext lint preflight..."
$lintOutput = & npx --yes web-ext lint --source-dir $source --output json 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Warning "web-ext lint returned exit code $LASTEXITCODE"
}
try {
    $lintResult = $lintOutput | ConvertFrom-Json
    $errorCount = $lintResult.summary.errors
    $warnCount = $lintResult.summary.warnings
    Write-Output "Linter Summary:    $errorCount errors, $warnCount warnings"
    if ($errorCount -gt 0) {
        throw "Extension has $errorCount lint error(s); cannot publish or sign."
    }
}
catch {
    if ($errorCount -gt 0) {
        throw
    }
    Write-Output "Linter Summary:    verified"
}

if ($Action -eq "Plan" -or $Action -eq "Status") {
    Write-Output ""
    Write-Output "Manual Mozilla Add-ons (AMO) signing instructions:"
    Write-Output "  1. Log into https://addons.mozilla.org/developers/"
    Write-Output "  2. Go to 'Submit a New Add-on' or 'Manage API Keys'."
    Write-Output "  3. To sign for self-distribution (unlisted channel):"
    Write-Output "     - Select 'On your own' (unlisted) distribution."
    Write-Output "     - Upload: $ZipPath"
    Write-Output "     - Automated review signs the .xpi in minutes."
    Write-Output "  4. To automate via this script:"
    Write-Output "     - Save AMO_JWT_ISSUER and AMO_JWT_SECRET to $CredentialFile"
    Write-Output "     - Run: pwsh -File scripts/publish-extension-firefox.ps1 -Action SignUnlisted -Execute"
    Write-Output ""
    Write-Output "No external network requests were made (Action: $Action)."
    return
}

# Require owner approval for external execution
if (-not $Execute) {
    throw "$Action contacts Mozilla Add-ons (AMO). Pass -Execute only after owner approval for that exact action."
}

if (-not $hasCredentials) {
    throw "Cannot execute $Action without AMO_JWT_ISSUER and AMO_JWT_SECRET. Configure them in $CredentialFile or via environment variables."
}

$channel = if ($Action -eq "SignUnlisted") { "unlisted" } else { "listed" }
Write-Output ""
Write-Output "Submitting extension to Mozilla Add-ons (AMO) via API..."
Write-Output "Channel: $channel"

$issuer = $values["AMO_JWT_ISSUER"]
$secret = $values["AMO_JWT_SECRET"]

$signArgs = @(
    "--yes",
    "web-ext",
    "sign",
    "--channel=$channel",
    "--api-key=$issuer",
    "--api-secret=$secret",
    "--source-dir=$source",
    "--artifacts-dir=$ArtifactsDir",
    "--timeout=300000"
)

& npx $signArgs
if ($LASTEXITCODE -ne 0) {
    throw "web-ext sign failed with exit code $LASTEXITCODE"
}

Write-Output ""
Write-Output "Mozilla Add-ons signing succeeded."
$signedAssets = @(Get-ChildItem -LiteralPath $ArtifactsDir -File -Filter "*.xpi" |
    Sort-Object LastWriteTime -Descending)

if ($signedAssets.Count -gt 0) {
    $latestXpi = $signedAssets[0]
    $canonicalXpi = Join-Path $ArtifactsDir "ghostlight-firefox-extension-v$version.signed.xpi"
    Copy-Item -LiteralPath $latestXpi.FullName -Destination $canonicalXpi -Force
    $xpiHash = (Get-FileHash -LiteralPath $canonicalXpi -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Output "Signed artifact: $canonicalXpi"
    Write-Output "Raw download:    $($latestXpi.FullName)"
    Write-Output "Size:            $((Get-Item -LiteralPath $canonicalXpi).Length) bytes"
    Write-Output "SHA-256:         $xpiHash"
}
