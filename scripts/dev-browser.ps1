<#
.SYNOPSIS
  Launch an isolated, disposable Chrome profile for Ghostlight dev-loop testing (ADR-0059).

.DESCRIPTION
  Starts Chrome with --user-data-dir pointed at a fresh, gitignored directory under this repo's
  own scratch area (never your real profile), loads the unpacked dev extension, and sets
  GHOSTLIGHT_DEBUG=1 in its environment -- so the browser-role relay it spawns writes debug
  state automatically (the mechanism already documented in crates/relay/src/main.rs::run_browser,
  just never actually exercised before ADR-0059).

  This profile has nothing else attached to it: it is safe to kill, reload the extension in, or
  close at any point without negotiating with your other open Chrome windows or other tools'
  Ghostlight sessions, unlike the repo's real, shared native-messaging registration.

.PARAMETER ExtensionPath
  Path to the unpacked extension directory. Defaults to .\extension relative to the repo root.

.PARAMETER ProfileDir
  Where to put the disposable profile. Defaults to a fresh directory under
  $env:TEMP\ghostlight-dev-browser (never reused across runs, so each session starts clean).

.PARAMETER ChromePath
  Path to chrome.exe. Auto-detected from the usual install locations if omitted.

.EXAMPLE
  .\scripts\dev-browser.ps1
  Launches a clean, disposable, debug-instrumented Chrome pointed at the dev extension.
#>
param(
    [string]$ExtensionPath,
    [string]$ProfileDir,
    [string]$ChromePath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $ExtensionPath) {
    $ExtensionPath = Join-Path $repoRoot "extension"
}
if (-not (Test-Path $ExtensionPath)) {
    throw "extension directory not found: $ExtensionPath"
}

if (-not $ProfileDir) {
    $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $ProfileDir = Join-Path $env:TEMP "ghostlight-dev-browser\profile-$stamp"
}
New-Item -ItemType Directory -Force -Path $ProfileDir | Out-Null

if (-not $ChromePath) {
    $candidates = @(
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe"
    )
    $ChromePath = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $ChromePath) {
        throw "chrome.exe not found in the usual locations; pass -ChromePath explicitly"
    }
}

Write-Host "Ghostlight dev-browser (ADR-0059)"
Write-Host "  chrome     $ChromePath"
Write-Host "  extension  $ExtensionPath"
Write-Host "  profile    $ProfileDir (disposable; delete freely)"
Write-Host "  debug      GHOSTLIGHT_DEBUG=1 (the browser-role relay will write debug state)"
Write-Host ""
Write-Host "This profile is isolated: safe to kill/reload without touching your other Chrome"
Write-Host "windows or other tools' Ghostlight sessions."

$env:GHOSTLIGHT_DEBUG = "1"
Start-Process -FilePath $ChromePath -ArgumentList @(
    "--user-data-dir=$ProfileDir",
    "--load-extension=$ExtensionPath",
    "--no-first-run",
    "--no-default-browser-check",
    "about:blank"
)
