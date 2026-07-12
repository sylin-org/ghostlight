<#
.SYNOPSIS
  One-command Ghostlight dev loop (ADR-0065): make the fresh build THE engine, or hand back.

.DESCRIPTION
  Ghostlight runs ONE stack (ADR-0065): one native host, one endpoint, one MCP entry. The
  "engine" is simply whichever ghostlight.exe currently holds the endpoint -- the installed
  release or the build you just made. This script swaps the engine:

    dev-loop.ps1            quiesce self-heal, stop the current engine, rebuild, start the
                            fresh build as the engine, wait until healthy.
    dev-loop.ps1 -Restore   stop the repo-built engine and start the installed release again
                            (if none is installed, the next client's self-heal finds nothing
                            to revive and reports the endpoint down -- run install first).

  Relays (your editor's agent relay, the browser's native-messaging relay) are NEVER killed:
  they are dumb pipes that reconnect to whoever owns the endpoint (ADR-0045 / ADR-0062).
  Running relay EXEs are renamed aside (Windows allows renaming a running image) so the build
  can write fresh binaries; deploy.lock files (ADR-0063) in every candidate engine directory
  keep relay self-heal from respawning the OLD engine during the swap window.

  Identification safety: only processes whose executable path is under this repo's target\
  directory or under the well-known install root (~\.ghostlight\bin) are ever stopped --
  never a bare taskkill by name.

.PARAMETER Restore
  Hand the endpoint back: stop the repo-built engine and start the newest installed release.

.PARAMETER Manifest
  Optional path to a policy manifest to start the engine with (e.g. examples\dev-live-test.json
  for governed live tests). Default: none -- the engine serves real use with the real config.

.PARAMETER TimeoutSec
  How long to wait for the engine to report healthy before giving up. Default 30.

.PARAMETER Configuration
  Cargo build profile: Release (default) or Debug.

.EXAMPLE
  .\scripts\dev-loop.ps1
.EXAMPLE
  .\scripts\dev-loop.ps1 -Manifest examples\dev-live-test.json
.EXAMPLE
  .\scripts\dev-loop.ps1 -Restore
#>
param(
    [switch]$Restore,
    [string]$Manifest = "",
    [int]$TimeoutSec = 30,
    [ValidateSet("Release", "Debug")]
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
$locks = @()
try {
    $profileFlag = if ($Configuration -eq "Release") { "--release" } else { "" }
    $targetDir = if ($Configuration -eq "Release") { "target\release" } else { "target\debug" }
    $ghostlightExe = Join-Path $repoRoot "$targetDir\ghostlight.exe"
    $relayExe = Join-Path $repoRoot "$targetDir\ghostlight-relay.exe"

    # -Restore only considers releases new enough for the one-stack swap (ADR-0065). A release
    # older than this floor predates the relay reconnect (ADR-0062) and the deploy.lock quiesce
    # (ADR-0063): its relays cannot see the lock and self-heal the OLD engine back mid-swap, and
    # its engine cannot parse the current extension's identity frame (ADR-0061). Restoring one
    # reintroduces exactly the endpoint fight the swap is designed to avoid.
    $minRestoreVersion = [version]"0.5.5"

    # Every directory that may hold an engine a relay could self-heal: the repo build dir plus
    # each versioned install dir (ALL of them, floor or not -- the lock is placed everywhere; the
    # floor only gates what -Restore will start). Sorted by parsed version, not name, so v0.5.10
    # outranks v0.5.9.
    $engineDirs = @((Join-Path $repoRoot $targetDir))
    $installRoot = Join-Path $env:USERPROFILE ".ghostlight\bin"
    $installed = @()
    if (Test-Path $installRoot) {
        $installed = @(Get-ChildItem $installRoot -Directory -ErrorAction SilentlyContinue |
            ForEach-Object {
                $v = $null
                if ($_.Name -match '^v(\d+\.\d+\.\d+)$' -and [version]::TryParse($Matches[1], [ref]$v)) {
                    $exe = Join-Path $_.FullName "ghostlight.exe"
                    if (Test-Path $exe) { [pscustomobject]@{ Version = $v; Exe = $exe } }
                }
            } |
            Sort-Object Version -Descending)
        $engineDirs += @($installed | ForEach-Object { Split-Path $_.Exe })
    }

    function Set-DeployLocks {
        foreach ($dir in $engineDirs) {
            if (Test-Path $dir) {
                $lock = Join-Path $dir "deploy.lock"
                Set-Content -Path $lock -Value "dev-loop $(Get-Date -Format o)" -Encoding ascii
                $script:locks += $lock
            }
        }
    }

    function Stop-Engines([string[]]$Roots) {
        # Stop SERVICE processes only (ghostlight.exe), never relays -- relays reconnect.
        $procs = Get-CimInstance Win32_Process -Filter "Name='ghostlight.exe'" |
            Where-Object {
                $p = $_.ExecutablePath
                $p -and ($Roots | Where-Object { $p.StartsWith($_, [StringComparison]::OrdinalIgnoreCase) })
            }
        foreach ($p in $procs) {
            Write-Host "  stopping engine pid $($p.ProcessId) ($($p.ExecutablePath))"
            Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
        }
        if ($procs) { Start-Sleep -Milliseconds 500 }
    }

    function Wait-Healthy([string]$Exe) {
        $deadline = (Get-Date).AddSeconds($TimeoutSec)
        while ((Get-Date) -lt $deadline) {
            $doctorOut = & $Exe doctor 2>&1 | Out-String
            if ($doctorOut -match "state\s+accepts connections") { return $true }
            Start-Sleep -Milliseconds 500
        }
        return $false
    }

    if ($Restore) {
        $candidate = $installed | Where-Object { $_.Version -ge $minRestoreVersion } | Select-Object -First 1
        if (-not $candidate) {
            $found = if ($installed) { ($installed | ForEach-Object { "v$($_.Version)" }) -join ", " } else { "none" }
            Write-Host "No installed release is one-stack capable (>= v$minRestoreVersion) under $installRoot (found: $found)."
            Write-Host "A pre-v$minRestoreVersion release predates the relay reconnect (ADR-0062) and the deploy.lock"
            Write-Host "quiesce (ADR-0063); restoring one would fight the swap and cannot talk to the current"
            Write-Host "extension. Install a current release first -- the repo build stays the engine for now."
            exit 1
        }
        Write-Host "[1/3] Quiescing self-heal and stopping the repo-built engine..."
        Set-DeployLocks
        Stop-Engines @($repoRoot)

        Write-Host "[2/3] Starting the installed engine: $($candidate.Exe) (v$($candidate.Version))"
        Start-Process -FilePath $candidate.Exe -ArgumentList @("service") -WindowStyle Hidden

        Write-Host "[3/3] Waiting up to ${TimeoutSec}s for the endpoint..."
        if (-not (Wait-Healthy $candidate.Exe)) {
            throw "the installed engine never reported healthy within ${TimeoutSec}s; run '$($candidate.Exe) doctor' by hand"
        }
        Write-Host "Restored: the installed release holds the endpoint again."
        exit 0
    }

    Write-Host "[1/5] Quiescing self-heal (deploy.lock in every engine dir) and moving relay EXEs aside..."
    Set-DeployLocks
    if (Test-Path $relayExe) {
        $aside = "$relayExe.$([System.Guid]::NewGuid().ToString('N')).old"
        try { Rename-Item -Path $relayExe -NewName (Split-Path $aside -Leaf) -Force } catch { Write-Host "  (relay.exe not moved: $($_.Exception.Message))" }
    }

    Write-Host "[2/5] Stopping the current engine (repo-built or installed; relays stay up)..."
    Stop-Engines (@($repoRoot) + @($installRoot | Where-Object { Test-Path $_ }))

    Write-Host "[3/5] Building ghostlight + ghostlight-relay + lightbox ($Configuration)..."
    if ($profileFlag) {
        cargo build $profileFlag -p ghostlight -p ghostlight-relay -p ghostlight-lightbox
    } else {
        cargo build -p ghostlight -p ghostlight-relay -p ghostlight-lightbox
    }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
    Get-ChildItem -Path (Split-Path $relayExe) -Filter "ghostlight-relay.exe.*.old" -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue }

    Write-Host "[4/5] Starting the fresh build as THE engine..."
    $serviceArgs = @("--debug", "service", "--keep-warm")
    if ($Manifest) {
        $manifestPath = Resolve-Path $Manifest
        $manifestUri = "file://" + ("$manifestPath" -replace '\\', '/')
        $serviceArgs = @("--debug", "--manifest", $manifestUri, "service", "--keep-warm")
        Write-Host "  (with policy manifest: $manifestPath)"
    }
    Start-Process -FilePath $ghostlightExe -ArgumentList $serviceArgs -WindowStyle Hidden

    Write-Host "[5/5] Waiting up to ${TimeoutSec}s for the endpoint to accept connections..."
    if (-not (Wait-Healthy $ghostlightExe)) {
        throw "the engine never reported healthy within ${TimeoutSec}s; run '$ghostlightExe doctor' by hand"
    }
    Write-Host "The fresh build holds the endpoint. Relays, editors, and browsers reconnect on their own."

    Write-Host ""
    Write-Host "Offline smoke check (lightbox fake-browser, no Chrome needed)..."
    $lightboxExe = Join-Path $repoRoot "$targetDir\lightbox.exe"
    if (Test-Path $lightboxExe) {
        "quit" | & $lightboxExe fake-browser --auto-reply
    } else {
        Write-Host "  (lightbox.exe not built at $lightboxExe; run 'cargo build $profileFlag -p ghostlight-lightbox' to add this check)"
    }

    Write-Host ""
    Write-Host "Ready. Next:"
    Write-Host "  - Status any time: $targetDir\ghostlight.exe doctor"
    Write-Host "  - Interactive protocol driving: $targetDir\lightbox.exe fake-browser --auto-reply"
    Write-Host "  - Hand the endpoint back to the installed release: .\scripts\dev-loop.ps1 -Restore"
    exit 0
}
finally {
    # Never leave self-heal quiesced, whether we succeeded or threw (the stale-lock guard would
    # eventually expire the locks, but releasing them now restores self-heal immediately).
    foreach ($lock in $locks) { Remove-Item -Path $lock -Force -ErrorAction SilentlyContinue }
    Pop-Location
}
