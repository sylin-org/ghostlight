<#
.SYNOPSIS
  Packages the Ghostlight browser extension into a Chrome-Web-Store-ready zip.

.DESCRIPTION
  Reads the version out of extension/manifest.json, stages the store-relevant
  extension files into a temp folder (excluding local-install/dev-only files),
  and zips the STAGED folder's contents -- not the folder itself -- to
  dist/ghostlight-extension-v<version>.zip, so manifest.json sits at the zip
  root as the Chrome Web Store requires.

.PARAMETER Version
  The expected Chrome adapter version. When supplied, it must match the version
  in extension/manifest.json. The manifest remains the source of truth.

.EXAMPLE
  pwsh -File scripts\package-extension.ps1

.EXAMPLE
  pwsh -File scripts\package-extension.ps1 -Version 0.2.0
#>
param(
  [string]$Version
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ExtensionDir = Join-Path $RepoRoot 'extension'
$ManifestPath = Join-Path $ExtensionDir 'manifest.json'
$DistDir = Join-Path $RepoRoot 'dist'

if (-not (Test-Path $ManifestPath)) {
  throw "manifest.json not found at $ManifestPath"
}
$manifest = Get-Content $ManifestPath -Raw | ConvertFrom-Json
$ManifestVersion = $manifest.version
if (-not $ManifestVersion) { throw "No version found in $ManifestPath." }
if ($Version -and $Version -ne $ManifestVersion) {
  throw "Requested Chrome adapter version $Version does not match manifest version $ManifestVersion."
}
$Version = $ManifestVersion

# Files not wanted in the store package: native-messaging-host.json is a
# local-install template, README.md is developer-facing docs, and the icon
# master + 512px hi-res asset + stale placeholder SVG are repo/source files the
# manifest never references (the extension ships only the 16/32/48/128 icons).
$ExcludeRelativePaths = @(
  'native-messaging-host.json',
  'README.md',
  'icons/mascot.png',
  'icons/icon512.png',
  'icons/ghost-mark.svg'
)

# Cross-platform temp root ($env:TEMP is Windows-only; this script also runs on the ubuntu
# release runner via pwsh, so it must not depend on it).
$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) "ghostlight-extension-stage-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $StageDir | Out-Null

try {
  Get-ChildItem -Path $ExtensionDir -Recurse -File | ForEach-Object {
    [pscustomobject]@{
      Full     = $_.FullName
      Relative = ($_.FullName.Substring($ExtensionDir.Length).TrimStart('\', '/')) -replace '\\', '/'
    }
  } | Where-Object { $ExcludeRelativePaths -notcontains $_.Relative } | ForEach-Object {
    $target = Join-Path $StageDir $_.Relative
    New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
    Copy-Item -Path $_.Full -Destination $target
  }

  # Strip the `key` field from the store manifest. The Chrome Web Store rejects a
  # `key` on first upload ("key field is not allowed in manifest") and manages the
  # extension id itself. The committed manifest keeps `key` for unpacked local dev
  # (pinned dev id); the store package must not carry it.
  $StagedManifest = Join-Path $StageDir 'manifest.json'
  $m = Get-Content $StagedManifest -Raw | ConvertFrom-Json
  $m.PSObject.Properties.Remove('key')
  $json = $m | ConvertTo-Json -Depth 20
  # ConvertTo-Json escapes <, >, & as \uXXXX; restore them so host_permissions like
  # <all_urls> stay readable (Chrome accepts either form).
  $json = $json -replace '\\u003c', '<' -replace '\\u003e', '>' -replace '\\u0026', '&'
  [System.IO.File]::WriteAllText($StagedManifest, $json + "`n")

  New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

  $ZipPath = Join-Path $DistDir "ghostlight-extension-v$Version.zip"
  if (Test-Path $ZipPath) { Remove-Item -Path $ZipPath -Force }

  # Zip the staged folder's CONTENTS (trailing \*), not the folder itself, so
  # manifest.json lands at the zip root -- the Chrome Web Store rejects a
  # manifest nested under a subfolder.
  Compress-Archive -Path (Join-Path $StageDir '*') -DestinationPath $ZipPath -Force
}
finally {
  Remove-Item -Path $StageDir -Recurse -Force -ErrorAction SilentlyContinue
}

$ZipPath = (Resolve-Path $ZipPath).Path
$sizeKb = [math]::Round((Get-Item $ZipPath).Length / 1KB, 1)
Write-Host "Packaged: $ZipPath ($sizeKb KB)" -ForegroundColor Green
