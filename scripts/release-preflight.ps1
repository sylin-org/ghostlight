# SPDX-License-Identifier: Apache-2.0 OR MIT

<#
.SYNOPSIS
    Run the complete local release-preflight gate list in order, with per-stage results.

.DESCRIPTION
    Encodes the locally runnable portion of the RELEASE-CHECKLIST G1 gate list as ordered
    stages. Each stage reports PASS, FAIL, or SKIP with captured output; by default the run
    stops at the first failing stage (-ContinueOnFailure overrides). The console shows each
    stage's result and the first line of its detail, so test counts and journey summaries stay
    visible; copy the source-gate evidence into the candidate custody record.

    Stages that cannot run on this host (shell syntax checks without a shell, dependency
    gates without cargo-deny installed) are recorded as SKIP rather than faked.

.PARAMETER SkipBuild
    Skip the isolated workspace build stage (reuse an existing target directory).

.PARAMETER SkipJourneys
    Skip the process/CLI/workbench journey stages.

.PARAMETER ContinueOnFailure
    Run every stage even after a failure instead of stopping at the first red.

.PARAMETER TargetDirectory
    Isolated build directory used by the build stage and the journeys.

.EXAMPLE
    pwsh scripts/release-preflight.ps1
#>

[CmdletBinding()]
param(
    [switch] $SkipBuild,
    [switch] $SkipJourneys,
    [switch] $ContinueOnFailure,
    [switch] $IncludeDependencyGates,
    [string] $TargetDirectory = ".target-ghostlight-1.0"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Push-Location $repository
try {
    # ---------------------------------------------------------------- helpers

    function Invoke-Stage {
        param(
            [Parameter(Mandatory)] [string] $Name,
            [Parameter(Mandatory)] [scriptblock] $Action,
            [string] $SkipReason = ""
        )
        if ($SkipReason) {
            Write-Host ("{0,-58} SKIP   {1}" -f $Name, $SkipReason)
            return @{ Name = $Name; Result = "SKIP"; Detail = $SkipReason; DurationMs = 0 }
        }
        Write-Host ("{0,-58} running" -f $Name)
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        try {
            $detail = @(& $Action) | ForEach-Object { "$_" } | Select-Object -Last 3
            $sw.Stop()
            Write-Host ("{0,-58} PASS   {1}ms" -f $Name, $sw.ElapsedMilliseconds)
            $firstDetail = ($detail | Select-Object -First 1)
            if ($firstDetail) { Write-Host ("    {0}" -f $firstDetail) }
            return @{
                Name       = $Name
                Result     = "PASS"
                Detail     = ($detail -join "; ")
                DurationMs = $sw.ElapsedMilliseconds
            }
        } catch {
            $sw.Stop()
            Write-Host ("{0,-58} FAIL   {1}" -f $Name, $_.Exception.Message)
            return @{
                Name       = $Name
                Result     = "FAIL"
                Detail     = $_.Exception.Message
                DurationMs = $sw.ElapsedMilliseconds
            }
        }
    }

    function Assert-LastExitZero {
        param([string] $FailureMessage)
        if ($LASTEXITCODE -ne 0) { throw $FailureMessage }
    }

    # Runs one native command, captures combined output, asserts exit zero, and surfaces the
    # output tail as stage detail.
    function Run-Gate {
        param([Parameter(Mandatory)] [scriptblock] $Command, [Parameter(Mandatory)] [string] $FailureMessage)
        $output = & $Command 2>&1
        if ($LASTEXITCODE -ne 0) {
            $tail = ($output | Select-Object -Last 6 | ForEach-Object { "$_" }) -join "`n"
            throw "$FailureMessage`n$tail"
        }
        $output | Select-Object -Last 2 | ForEach-Object { "$_" }
    }

    # The machine's persistent native-host registration as one comparable string. Recovery
    # silently repairs Ghostlight-owned registrations toward the running tree (ADR-0149), so a
    # journey that is not registration-isolated would adopt the real browsers into the build
    # under test -- the 2026-08-30 preflight leak. Journeys isolate via
    # GHOSTLIGHT_NATIVE_HOST_DIR; this snapshot proves it held.
    function Get-MachineNativeHostState {
        $parts = @()
        if ($IsWindows) {
            $manifest = Join-Path $env:USERPROFILE ".ghostlight\manifests\org.sylin.ghostlight.json"
            $parts += if (Test-Path -LiteralPath $manifest) {
                (Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash
            } else { "<absent>" }
            foreach ($vendor in @(
                , @("Google", "Chrome")
                , @("Microsoft", "Edge")
                , @("BraveSoftware", "Brave-Browser")
                , @("Chromium")
            )) {
                $key = "Registry::HKEY_CURRENT_USER\Software\$($vendor -join '\')\NativeMessagingHosts\org.sylin.ghostlight"
                $value = (Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue)."(default)"
                $parts += if ($null -ne $value) { "$value" } else { "<absent>" }
            }
        } else {
            $configHome = if ($env:XDG_CONFIG_HOME) { $env:XDG_CONFIG_HOME } else { Join-Path $HOME ".config" }
            foreach ($directory in @("google-chrome", "microsoft-edge", "BraveSoftware/Brave-Browser", "chromium")) {
                $manifest = Join-Path $configHome "$directory/NativeMessagingHosts/org.sylin.ghostlight.json"
                $parts += if (Test-Path -LiteralPath $manifest) {
                    (Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash
                } else { "<absent>" }
            }
        }
        return ($parts -join "`n---`n")
    }

    # Ghostlight processes still running from this runner's isolated target directory. Nothing
    # legitimate runs from a scratch build tree, so the guard stops them and reports them.
    function Get-LeakedTargetProcesses {
        param([Parameter(Mandatory)] [string] $TargetRoot)
        if ($IsWindows) {
            $result = @(
                Get-CimInstance Win32_Process -Filter "Name like 'ghostlight%'" -ErrorAction SilentlyContinue |
                    Where-Object {
                        $_.ExecutablePath -and
                        $_.ExecutablePath.StartsWith($TargetRoot, [System.StringComparison]::OrdinalIgnoreCase)
                    }
            )
        } else {
            $result = @(ps -eo pid=,args= | Where-Object { "$_" -like "*$TargetRoot*ghostlight*" } |
                ForEach-Object { "$_" -replace '^\s*(\d+)\s+.*$', '$1' })
        }
        # The comma keeps the array intact through the function's output pipeline, so an empty
        # result is still an array and .Count stays legal under StrictMode.
        return , $result
    }

    # ---------------------------------------------------------------- environment

    $headSha = (git rev-parse HEAD).Trim()
    $dirtyLines = (git status --porcelain | Measure-Object -Line).Lines
    $hasShell = [bool](Get-Command sh -ErrorAction SilentlyContinue)

    $targetDirectory = [System.IO.Path]::GetFullPath((Join-Path $repository $TargetDirectory))
    $binDirectory = Join-Path $targetDirectory "debug"

    # ---------------------------------------------------------------- stage list

    $stages = @()

    $stages += @{ Name = "cargo fmt --all -- --check"; Skip = ""; Action = {
        Run-Gate { cargo fmt --all -- --check } "formatting drift"
    } }

    $stages += @{ Name = "cargo test --workspace"; Skip = ""; Action = {
        Run-Gate { cargo test --workspace } "workspace tests failed"
    } }

    $stages += @{ Name = "extension tests (npm test)"; Skip = ""; Action = {
        Push-Location extension
        try {
            Run-Gate { npm test } "extension tests failed"
        } finally { Pop-Location }
    } }

    $stages += @{ Name = "Chromium page-asset tests"; Skip = ""; Action = {
        Run-Gate { node --test "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js" } "page-asset tests failed"
    } }

    $stages += @{ Name = "npm launcher tests (packaging/npm)"; Skip = ""; Action = {
        Push-Location packaging/npm
        try {
            Run-Gate { npm test } "launcher tests failed"
        } finally { Pop-Location }
    } }

    $stages += @{ Name = "MCPB launcher tests"; Skip = ""; Action = {
        Push-Location packaging/mcpb
        try {
            Run-Gate { node --test test/launcher.test.js } "MCPB tests failed"
        } finally { Pop-Location }
    } }

    $stages += @{
        Name   = "shell script syntax (sh -n scripts/*.sh)"
        Skip   = if ($hasShell) { "" } else { "no shell on this host; CI runs sh -n" }
        Action = {
            Run-Gate { sh -c 'for f in scripts/*.sh; do sh -n "$f" || exit 1; done' } "shell syntax failed"
        }
    }

    if ($SkipBuild) {
        $stages += @{ Name = "isolated workspace build"; Skip = "skipped by request"; Action = {} }
    } else {
        $stages += @{ Name = "isolated workspace build ($TargetDirectory)"; Skip = ""; Action = {
            Run-Gate { cargo build --workspace --target-dir $TargetDirectory } "isolated build failed"
        } }
    }

    # Each journey pins GHOSTLIGHT_BIN_DIR to THIS runner's isolated target at execution time,
    # so a -TargetDirectory run can never silently verify stale binaries from the default
    # location (CachyOS finding 1, 2026-08-25).
    $previousBinDir = $env:GHOSTLIGHT_BIN_DIR
    if ($SkipJourneys) {
        $stages += @{ Name = "machine registration snapshot"; Skip = "skipped with the journeys"; Action = {} }
        $stages += @{ Name = "journeys (process/CLI/PowerShell/workbench)"; Skip = "skipped by request"; Action = {} }
        $stages += @{ Name = "machine state guard (registration + leaked processes)"; Skip = "skipped with the journeys"; Action = {} }
    } else {
        $stages += @{ Name = "machine registration snapshot"; Skip = ""; Action = {
            $script:journeyRegistrationSnapshot = Get-MachineNativeHostState
            "real native-host registration captured before the journeys"
        } }

        $stages += @{ Name = "process journey"; Skip = ""; Action = {
            $env:GHOSTLIGHT_BIN_DIR = $binDirectory
            Run-Gate { node tests/process-journey.mjs } "process journey failed"
        } }
        $stages += @{ Name = "CLI journey"; Skip = ""; Action = {
            $env:GHOSTLIGHT_BIN_DIR = $binDirectory
            Run-Gate { node tests/cli-journey.mjs } "CLI journey failed"
        } }
        $stages += @{ Name = "CLI PowerShell journey"; Skip = ""; Action = {
            $env:GHOSTLIGHT_BIN_DIR = $binDirectory
            Run-Gate { node tests/cli-powershell-journey.mjs } "PowerShell journey failed"
        } }
        $stages += @{ Name = "workbench surface"; Skip = ""; Action = {
            $env:GHOSTLIGHT_BIN_DIR = $binDirectory
            Run-Gate { node tests/workbench-surface.mjs } "workbench surface failed"
        } }

        # The guarantee stage (2026-08-30 leak): the journeys leave the machine's registration
        # untouched and leave no process behind inside the isolated target. Leaked processes are
        # stopped (nothing legitimate runs from a scratch tree) and still fail the gate so the
        # leak is fixed at its seam, not cleaned silently.
        $stages += @{ Name = "machine state guard (registration + leaked processes)"; Skip = ""; Action = {
            $after = Get-MachineNativeHostState
            if ($after -ne $script:journeyRegistrationSnapshot) {
                throw "the journeys changed the machine's native-host registration"
            }
            $leaked = Get-LeakedTargetProcesses -TargetRoot $targetDirectory
            foreach ($leak in $leaked) {
                if ($IsWindows) {
                    Stop-Process -Id $leak.ProcessId -Force -ErrorAction SilentlyContinue
                } else {
                    & kill -9 $leak 2> $null
                }
            }
            if ($leaked.Count -gt 0) {
                $described = ($leaked | ForEach-Object {
                    if ($IsWindows) { "$($_.ProcessId): $($_.ExecutablePath)" } else { "pid $_" }
                }) -join "; "
                throw "the journeys left ghostlight processes inside $TargetDirectory -- $described"
            }
            "registration unchanged; no process leaked from the isolated target"
        } }
    }

    $stages += @{ Name = "policy grammar"; Skip = ""; Action = {
        Run-Gate { node tests/policy-grammar.mjs } "policy grammar failed"
    } }

    $stages += @{ Name = "JavaScript syntax (extension, orchestrator, preview server)"; Skip = ""; Action = {
        $scripts = @(git ls-files 'extension/*.js' 'crates/orchestrator/*.js') |
            Where-Object { $_ -notlike '*/tests/*' -and $_ -notlike '*/vendor/*' }
        foreach ($script in $scripts) {
            node --check $script
            if ($LASTEXITCODE -ne 0) { throw "$script syntax" }
        }
        "$($scripts.Count) files checked"
        node --check tests/workbench-preview-server.mjs
        if ($LASTEXITCODE -ne 0) { throw "preview server syntax" }
    } }

    $missingDependencyTools = @()
    if (-not (Get-Command cargo-deny -ErrorAction SilentlyContinue)) { $missingDependencyTools += "cargo-deny" }
    if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) { $missingDependencyTools += "cargo-audit" }
    $dependencySkip = if ($missingDependencyTools.Count -gt 0) {
        "$($missingDependencyTools -join ' and ') not installed; CI runs license/ban/source/advisory"
    } elseif (-not $IncludeDependencyGates) {
        "run with -IncludeDependencyGates (or by CI)"
    } else {
        ""
    }

    # The authoritative split from RELEASE.md: policy checks via deny's non-advisory tables, and
    # advisories through cargo audit, whose configuration carries the accepted GTK/Tauri-chain
    # allowances. A broad `cargo deny check` would fail on that accepted set (CachyOS finding 2,
    # 2026-08-25).
    $stages += @{
        Name   = "dependency gates (deny licenses/bans/sources + audit)"
        Skip   = $dependencySkip
        Action = {
            Run-Gate { cargo deny check licenses bans sources } "dependency policy failed"
            Run-Gate { cargo audit } "dependency advisories failed"
        }
    }

    # ---------------------------------------------------------------- execution

    Write-Host ""
    Write-Host "Release preflight -- source revision $headSha$(if ($dirtyLines -gt 0) { ' (dirty tree)' })"
    Write-Host ""

    $results = @()
    foreach ($stage in $stages) {
        $results += Invoke-Stage -Name $stage.Name -Skip $stage.Skip -Action $stage.Action
        $lastResult = $results[-1]
        if ($lastResult.Result -eq "FAIL" -and -not $ContinueOnFailure) { break }
    }

    # Restore the caller's environment now that every stage has run.
    if ($previousBinDir) {
        $env:GHOSTLIGHT_BIN_DIR = $previousBinDir
    } else {
        Remove-Item Env:GHOSTLIGHT_BIN_DIR -ErrorAction SilentlyContinue
    }

    Write-Host ""
    $failures = @($results | Where-Object Result -eq "FAIL")
    foreach ($failure in $failures) {
        Write-Host ("FAILED: {0} -- {1}" -f $failure.Name, ($failure.Detail -split "`n")[0])
    }
    $passed = @($results | Where-Object Result -eq "PASS").Count
    $skipped = @($results | Where-Object Result -eq "SKIP").Count
    Write-Host ("Stages: {0} passed, {1} failed, {2} skipped." -f $passed, $failures.Count, $skipped)

    if ($failures.Count -gt 0) { exit 1 }
    exit 0
}
finally {
    Pop-Location
}
