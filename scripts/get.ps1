# SPDX-License-Identifier: Apache-2.0 OR MIT
# Ghostlight one-line installer (Windows):
#   irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1 | iex
# Downloads the latest release binary, places it in %USERPROFILE%\.ghostlight\bin, and runs
# `ghostlight install` (idempotent: registers the native messaging host and any MCP clients
# it finds). Safe to re-run. Set $env:GHOSTLIGHT_NO_REGISTER = "1" to download only.

$ErrorActionPreference = "Stop"

$Repo = "sylin-org/ghostlight"
$InstallPage = "https://sylin-org.github.io/ghostlight/install.html"

if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Error "ghostlight: only x64 Windows has a prebuilt binary. See $InstallPage"
}

$BinDir = Join-Path $env:USERPROFILE ".ghostlight\bin"
$Bin = Join-Path $BinDir "ghostlight.exe"
$Url = "https://github.com/$Repo/releases/latest/download/ghostlight-x86_64-pc-windows-msvc.exe"

New-Item -ItemType Directory -Force $BinDir | Out-Null
Write-Host "ghostlight: downloading latest release..."
$Tmp = "$Bin.download"
Invoke-WebRequest -Uri $Url -OutFile $Tmp -UseBasicParsing
Move-Item -Force $Tmp $Bin
$Version = try { & $Bin --version } catch { "version unknown" }
Write-Host "ghostlight: installed to $Bin ($Version)"

if ($env:GHOSTLIGHT_NO_REGISTER -ne "1") {
    Write-Host "ghostlight: registering (native messaging host + detected MCP clients)..."
    & $Bin install
}

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$BinDir*") {
    Write-Host "ghostlight: tip: add $BinDir to your PATH for the ghostlight CLI (doctor, config, policy)."
}

Write-Host ""
Write-Host "Next: add the 'Ghostlight in Browser' extension, then reload your MCP client."
Write-Host "Walkthrough: $InstallPage"
