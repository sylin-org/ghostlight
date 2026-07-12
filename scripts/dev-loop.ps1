<#
.SYNOPSIS
  One-command Ghostlight dev loop (ADR-0059): kill, rebuild, restart, verify.

.DESCRIPTION
  Replaces the manual dance this project's own live-verification sessions have repeatedly
  needed: stop this repo's own stray dev-instance processes (never anything outside its own
  target dir -- it identifies processes by their exact executable path, never a bare taskkill
  by name), rebuild ghostlight + ghostlight-relay, restart the dev service with the committed
  examples/dev-live-test.json fixture, poll `ghostlight doctor` until the endpoint is healthy
  (bounded by -TimeoutSec), and run one offline smoke check with `lightbox fake-browser` to
  confirm the wire protocol admits a session -- all without needing a real Chrome window.

  For an actual browser round trip afterward, use .\scripts\dev-browser.ps1 separately: this
  script only proves the SERVICE side is healthy.

.PARAMETER TimeoutSec
  How long to wait for the dev service to report healthy before giving up. Default 30.

.PARAMETER Configuration
  Cargo build profile: Release (default) or Debug.

.EXAMPLE
  .\scripts\dev-loop.ps1
#>
param(
    [int]$TimeoutSec = 30,
    [ValidateSet("Release", "Debug")]
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $profileFlag = if ($Configuration -eq "Release") { "--release" } else { "" }
    $targetDir = if ($Configuration -eq "Release") { "target\release" } else { "target\debug" }
    $ghostlightExe = Join-Path $repoRoot "$targetDir\ghostlight.exe"
    $relayExe = Join-Path $repoRoot "$targetDir\ghostlight-relay.exe"
    $fixture = Join-Path $repoRoot "examples\dev-live-test.json"

    Write-Host "[1/4] Stopping this repo's own dev-instance processes..."
    $mine = Get-CimInstance Win32_Process -Filter "Name='ghostlight.exe' OR Name='ghostlight-relay.exe'" |
        Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($repoRoot, [StringComparison]::OrdinalIgnoreCase) }
    foreach ($p in $mine) {
        Write-Host "  stopping pid $($p.ProcessId) ($($p.ExecutablePath))"
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 500

    Write-Host "[2/4] Building ghostlight + ghostlight-relay + lightbox ($Configuration)..."
    if ($profileFlag) {
        cargo build $profileFlag -p ghostlight -p ghostlight-relay -p ghostlight-lightbox
    } else {
        cargo build -p ghostlight -p ghostlight-relay -p ghostlight-lightbox
    }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }

    Write-Host "[3/4] Starting the dev service with examples\dev-live-test.json..."
    $manifestUri = "file://" + ($fixture -replace '\\', '/')
    Start-Process -FilePath $ghostlightExe -ArgumentList @(
        "--debug", "--instance", "dev", "--manifest", $manifestUri, "service", "--keep-warm"
    ) -WindowStyle Hidden

    Write-Host "[4/4] Waiting up to ${TimeoutSec}s for the dev endpoint to accept connections..."
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $healthy = $false
    while ((Get-Date) -lt $deadline) {
        $doctorOut = & $ghostlightExe --instance dev doctor 2>&1 | Out-String
        if ($doctorOut -match "state\s+accepts connections") {
            $healthy = $true
            break
        }
        Start-Sleep -Milliseconds 500
    }
    if (-not $healthy) {
        throw "dev service never reported healthy within ${TimeoutSec}s; run '$ghostlightExe --instance dev doctor' by hand"
    }
    Write-Host "dev service is up."

    Write-Host ""
    Write-Host "Offline smoke check (lightbox fake-browser, no Chrome needed)..."
    $lightboxExe = Join-Path $repoRoot "$targetDir\lightbox.exe"
    if (Test-Path $lightboxExe) {
        "quit" | & $lightboxExe fake-browser --instance dev --auto-reply
    } else {
        Write-Host "  (lightbox.exe not built at $lightboxExe; run 'cargo build $profileFlag -p ghostlight-lightbox' to add this check)"
    }

    Write-Host ""
    Write-Host "Ready. Next:"
    Write-Host "  - Real browser: .\scripts\dev-browser.ps1"
    Write-Host "  - Interactive protocol driving: $targetDir\lightbox.exe fake-browser --instance dev --auto-reply"
    Write-Host "  - Status any time: $targetDir\ghostlight.exe --instance dev doctor"
    exit 0
}
finally {
    Pop-Location
}
