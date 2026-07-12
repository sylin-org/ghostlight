# SPDX-License-Identifier: Apache-2.0 OR MIT
# Ghostlight one-line installer (Windows):
#   irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1 | iex
# Downloads the latest release binary, places it in %USERPROFILE%\.ghostlight\bin, and runs
# `ghostlight install` (idempotent: registers the native messaging host and any MCP clients
# it finds). Safe to re-run. Set $env:GHOSTLIGHT_NO_REGISTER = "1" to download only.

$ErrorActionPreference = "Stop"

$Repo = "sylin-org/ghostlight"
$InstallPage = "https://sylin.org/ghostlight/"

if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Error "ghostlight: only x64 Windows has a prebuilt binary. See $InstallPage"
}

$BinDir = Join-Path $env:USERPROFILE ".ghostlight\bin"
$Bin = Join-Path $BinDir "ghostlight.exe"

# Download one release binary and VERIFY it before trusting it (SEC-MED-06). These installers
# register a native-messaging host wired to your browser, so an unverified binary is high-impact.
# Mandatory gate: SHA-256 against the release's published checksum (catches corruption and
# transport tampering); the install refuses on a mismatch. Best-effort escalation: cryptographic
# build provenance via `gh attestation verify`, which -- unlike a co-located checksum -- a
# release-asset swap cannot forge; a gh miss is a warning, not a stop. The manual installer
# verifies provenance unconditionally (see $InstallPage).
function Install-VerifiedBinary([string]$Name) {
    $Url = "https://github.com/$Repo/releases/latest/download/$Name-x86_64-pc-windows-msvc.exe"
    $Dest = Join-Path $BinDir "$Name.exe"
    $Tmp = "$Dest.download"
    Invoke-WebRequest -Uri $Url -OutFile $Tmp -UseBasicParsing

    $Expected = (((Invoke-WebRequest -Uri "$Url.sha256" -UseBasicParsing).Content.Trim() -split '\s+')[0]).ToLower()
    $Actual = (Get-FileHash -Algorithm SHA256 $Tmp).Hash.ToLower()
    if (-not $Expected -or $Expected -ne $Actual) {
        Remove-Item -Force $Tmp
        Write-Error "ghostlight: checksum verification failed for $Name (expected '$Expected', got '$Actual'); aborting."
    }

    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if ($gh) {
        $null = & gh attestation verify $Tmp --repo $Repo 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  ${Name}: sha256 + build provenance verified"
        } else {
            Write-Host "  ${Name}: sha256 verified (gh could not confirm provenance; verify manually via $InstallPage)"
        }
    } else {
        Write-Host "  ${Name}: sha256 verified (install GitHub CLI 'gh' to also verify cryptographic build provenance)"
    }

    Move-Item -Force $Tmp $Dest
}

New-Item -ItemType Directory -Force $BinDir | Out-Null
Write-Host "ghostlight: downloading latest release..."
# ADR-0046 + ADR-0051 Phase 3: two executables ship together (the ghostlight brain + the single
# ghostlight-relay pass-through). They sit in one dir, so `ghostlight install` finds the relay sibling.
foreach ($b in "ghostlight", "ghostlight-relay") {
    Install-VerifiedBinary $b
}
$Version = try { & $Bin --version } catch { "version unknown" }
Write-Host "ghostlight: installed to $BinDir ($Version)"

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
