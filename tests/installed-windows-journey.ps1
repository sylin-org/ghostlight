#Requires -Version 7.0
<#
.SYNOPSIS
Exercises the real installed Chrome/native-host/service path, then every MCP tool.
.DESCRIPTION
Opt-in, disruptive integration test for an idle development installation. Removes and restores
the owned native-host registration, stops the exact browser connector, and crashes the exact
authority to prove demand-start. Chrome stays running. No browser settings or policy are changed.
Requires the normal four owned registrations. Refuses environment redirection and foreign paths.
Run on a dedicated release test machine for candidate acceptance; a dev install is not a package.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$BinDir,
    [Parameter(Mandatory)][string]$ChromePath,
    [Parameter(Mandatory)][switch]$ExerciseInstalledStack
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or -not $ExerciseInstalledStack) { throw 'Requires explicit Windows installed-stack mode.' }
$repo = Split-Path -Parent $PSScriptRoot
$binRoot = (Resolve-Path -LiteralPath $BinDir).Path
$chromeImage = (Resolve-Path -LiteralPath $ChromePath).Path
$authorityImage = Join-Path $binRoot 'ghostlight.exe'
$connectorImage = Join-Path $binRoot 'ghostlight-browser-connector.exe'
foreach ($name in @('GHOSTLIGHT_RUNTIME_FILE', 'GHOSTLIGHT_NATIVE_HOST_DIR', 'GHOSTLIGHT_PROFILE_DIR')) {
    if ([Environment]::GetEnvironmentVariable($name)) { throw "Remove ${name}: this journey requires the actual installation." }
}
$output = Join-Path $repo ('.tmp/installed-windows/' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfff'))
$null = New-Item -ItemType Directory -Path $output
$report = [ordered]@{
    started_at = [DateTime]::UtcNow.ToString('o'); passed = $false; release_ready = $false
    scope = 'Installed Windows developer stack; real Chrome and native messaging'
    revision = (& git -C $repo rev-parse HEAD); binaries = @{}; phases = @()
    remaining_release_gates = @('clean-install-both-orders', 'packaged-upgrade-uninstall',
        'store-adapter', 'second-browser', 'three-real-MCP-clients', 'visible-Linux')
}
function Save-Evidence {
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $output 'results.json')
}
function Invoke-Product([string[]]$Arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $authorityImage
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $info
    try {
        $null = $process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(15000)) { $process.Kill(); throw 'Product CLI timed out.' }
        if ($process.ExitCode -ne 0) { throw ('Product CLI failed: ' + $stderr.GetAwaiter().GetResult()) }
        return $stdout.GetAwaiter().GetResult()
    } finally { $process.Dispose() }
}
function Get-Doctor { (Invoke-Product @('doctor', '--json')) | ConvertFrom-Json }
function Get-ExactProcess([string]$Image) {
    @(Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Image)) -ErrorAction SilentlyContinue |
        Where-Object Path -EQ $Image)
}
function Get-ChromeRoot {
    @(Get-CimInstance Win32_Process -Filter "name='chrome.exe'" |
        Where-Object { $_.ExecutablePath -eq $chromeImage -and $_.CommandLine -notmatch '--type=' })
}
function Wait-Readiness([string]$Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    do {
        $doctor = Get-Doctor
        if ($null -ne $doctor.readiness -and $doctor.readiness.state -eq $Expected) { return }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Readiness did not become $Expected within 45 seconds."
}
function Assert-ChromeUnchanged {
    $roots = @(Get-ChromeRoot)
    if ($roots.Count -ne 1 -or $roots[0].ProcessId -ne $script:chromeProcessId -or
        $roots[0].CreationDate -ne $script:chromeCreated) { throw 'Chrome restarted during the journey.' }
}
function Stop-Exact($Observed, [string]$Image) {
    $current = Get-Process -Id $Observed.Id -ErrorAction Stop
    if ($current.Path -ne $Image -or $current.StartTime -ne $Observed.StartTime) { throw 'Process identity changed before stop.' }
    Stop-Process -InputObject $current -ErrorAction Stop
}
function Phase([string]$Name, [long]$Elapsed) {
    Assert-ChromeUnchanged
    $report.phases += @{ name = $Name; status = 'passed'; elapsed_ms = $Elapsed }
    Save-Evidence; Write-Host "PASS installed: $Name ($Elapsed ms)"
}
function Invoke-Mcp([int]$RequestId, [string]$Method, [hashtable]$Parameters) {
    $frame = @{ jsonrpc = '2.0'; id = $RequestId; method = $Method; params = $Parameters }
    $script:mcp.StandardInput.WriteLine(($frame | ConvertTo-Json -Compress -Depth 10))
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $line = $script:mcp.StandardOutput.ReadLineAsync()
        $remaining = [int][Math]::Max(1, ($deadline - [DateTime]::UtcNow).TotalMilliseconds)
        if (-not $line.Wait($remaining)) { throw 'Initialized MCP connection timed out.' }
        if ($null -eq $line.Result) { throw 'Initialized MCP connection closed.' }
        $reply = $line.Result | ConvertFrom-Json -AsHashtable
        if ($reply.ContainsKey('id') -and $reply.id -eq $RequestId) {
            if ($reply.ContainsKey('error')) { throw 'Initialized MCP connection returned a protocol error.' }
            return $reply.result
        }
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Initialized MCP connection did not return the requested response.'
}
$restoreOwed = $false
$registrations = @{}
$script:mcp = $null
try {
    Save-Evidence
    foreach ($name in @('ghostlight.exe', 'ghostlight-browser-connector.exe', 'ghostlight-mcp-connector.exe')) {
        $path = Join-Path $binRoot $name
        $report.binaries[$name] = @{ path = $path; sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
    }
    $roots = @(Get-ChromeRoot)
    if ($roots.Count -ne 1) { throw 'Requires one running installed Chrome root.' }
    $script:chromeProcessId = $roots[0].ProcessId; $script:chromeCreated = $roots[0].CreationDate
    $report.chrome_pid = $script:chromeProcessId
    $report.chrome_version = (Get-Item -LiteralPath $chromeImage).VersionInfo.ProductVersion
    if ((Get-Doctor).readiness.state -ne 'ready') { throw 'Requires an idle, Ready installation.' }
    $authorities = @(Get-ExactProcess $authorityImage); $connectors = @(Get-ExactProcess $connectorImage)
    if ($authorities.Count -ne 1 -or $connectors.Count -ne 1) { throw 'Requires one exact installed authority and connector.' }
    $authority = $authorities[0]
    $keys = @('Google/Chrome', 'Microsoft/Edge', 'BraveSoftware/Brave-Browser', 'Chromium') |
        ForEach-Object { "HKCU:/Software/$_/NativeMessagingHosts/org.sylin.ghostlight" }
    foreach ($key in $keys) {
        $registryKey = Get-Item -LiteralPath $key
        if ($registryKey.GetSubKeyNames().Count -ne 0 -or
            $registryKey.GetValueNames().Count -ne 1 -or $registryKey.GetValueNames()[0] -ne '' -or
            $registryKey.GetValueKind('') -ne [Microsoft.Win32.RegistryValueKind]::String) {
            throw 'Refusing a native-host key containing additional or nonstandard state.'
        }
        $path = $registryKey.GetValue('')
        $expectedManifest = Join-Path $env:LOCALAPPDATA 'Ghostlight/NativeMessagingHosts/org.sylin.ghostlight.json'
        if ([IO.Path]::GetFullPath($path) -ne [IO.Path]::GetFullPath($expectedManifest)) {
            throw 'Requires the ordinary per-user manifest path.'
        }
        $manifest = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        if ($manifest.name -ne 'org.sylin.ghostlight' -or $manifest.path -ne $connectorImage) {
            throw 'Registration is foreign or belongs to another installation.'
        }
        $registrations[$key] = @{ path = $path; hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash;
            bytes = [IO.File]::ReadAllBytes($path) }
    }
    $restoreOwed = $true
    $null = Invoke-Product @('native-host', 'uninstall')
    foreach ($key in $keys) { if (Test-Path -LiteralPath $key) { throw 'Registration was not removed.' } }
    Stop-Exact $connectors[0] $connectorImage
    Wait-Readiness 'not_connected'
    Start-Sleep -Seconds 5
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $null = Invoke-Product @('native-host', 'install')
    Wait-Readiness 'ready'
    foreach ($key in $keys) {
        if ((Get-Item -LiteralPath $key).GetValue('') -ne $registrations[$key].path -or
            (Get-FileHash -LiteralPath $registrations[$key].path -Algorithm SHA256).Hash -ne $registrations[$key].hash) {
            throw 'Restored registration differs from its initial state.'
        }
    }
    $restoreOwed = $false
    if ((Get-ExactProcess $authorityImage)[0].Id -ne $authority.Id) { throw 'Authority restarted during registration recovery.' }
    Phase 'registration-reinstalled-with-Chrome-running' $timer.ElapsedMilliseconds

    $connector = @(Get-ExactProcess $connectorImage)[0]
    Stop-Exact $connector $connectorImage
    $timer.Restart()
    do {
        Start-Sleep -Milliseconds 250
        $replacement = @(Get-ExactProcess $connectorImage)
    } while (($replacement.Count -ne 1 -or $replacement[0].Id -eq $connector.Id) -and $timer.Elapsed.TotalSeconds -lt 45)
    if ($replacement.Count -ne 1 -or $replacement[0].Id -eq $connector.Id) { throw 'Chrome did not replace its crashed connector.' }
    Wait-Readiness 'ready'
    Phase 'native-connector-crash-recovers-automatically' $timer.ElapsedMilliseconds

    $connector = $replacement[0]
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = Join-Path $binRoot 'ghostlight-mcp-connector.exe'
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true; $info.RedirectStandardOutput = $true
    $script:mcp = [Diagnostics.Process]::new(); $script:mcp.StartInfo = $info
    $null = $script:mcp.Start()
    $initialized = Invoke-Mcp 1 'initialize' @{ protocolVersion = '2025-11-25'; capabilities = @{};
        clientInfo = @{ name = 'Installed recovery acceptance'; version = '1' } }
    if ($initialized.serverInfo.name -ne 'ghostlight') { throw 'Unexpected MCP server identity.' }
    $script:mcp.StandardInput.WriteLine('{"jsonrpc":"2.0","method":"notifications/initialized"}')
    $listed = Invoke-Mcp 2 'tools/call' @{ name = 'browser_tabs'; arguments = @{ action = 'list' } }
    if ($listed.structuredContent.status -ne 'succeeded') { throw 'MCP browser work failed before service interruption.' }
    $report.retained_mcp_pid = $script:mcp.Id
    Stop-Exact $authority $authorityImage
    $timer.Restart()
    Wait-Readiness 'ready'
    $restarted = @(Get-ExactProcess $authorityImage)
    if ($restarted.Count -ne 1 -or $restarted[0].Id -eq $authority.Id) { throw 'Authority was not demand-started.' }
    $retained = @(Get-ExactProcess $connectorImage)
    if ($retained.Count -ne 1 -or $retained[0].Id -ne $connector.Id) { throw 'Native port was replaced during authority recovery.' }
    $listed = Invoke-Mcp 3 'tools/call' @{ name = 'browser_tabs'; arguments = @{ action = 'list' } }
    if ($listed.structuredContent.status -ne 'succeeded' -or $script:mcp.HasExited) {
        throw 'Same initialized MCP stream did not complete browser work after recovery.'
    }
    Phase 'authority-crash-recovers-with-same-native-port' $timer.ElapsedMilliseconds
    $script:mcp.StandardInput.Close()
    if (-not $script:mcp.WaitForExit(3000)) { $script:mcp.Kill() }
    $script:mcp.Dispose(); $script:mcp = $null

    $oldBin = $env:GHOSTLIGHT_BIN_DIR; $oldEvidence = $env:GHOSTLIGHT_LIVE_EVIDENCE
    try {
        $env:GHOSTLIGHT_BIN_DIR = $binRoot
        $env:GHOSTLIGHT_LIVE_EVIDENCE = Join-Path $output 'browser.json'
        & node (Join-Path $PSScriptRoot 'live-journey.mjs') 2>&1 | Tee-Object -FilePath (Join-Path $output 'browser.log')
        if ($LASTEXITCODE -ne 0) { throw 'Installed browser journey failed; see browser.log.' }
        $live = Get-Content -LiteralPath $env:GHOSTLIGHT_LIVE_EVIDENCE -Raw | ConvertFrom-Json
        if (-not $live.passed) { throw 'Installed browser evidence is incomplete.' }
    } finally { $env:GHOSTLIGHT_BIN_DIR = $oldBin; $env:GHOSTLIGHT_LIVE_EVIDENCE = $oldEvidence }
    foreach ($name in $report.binaries.Keys) {
        if ((Get-FileHash -LiteralPath $report.binaries[$name].path -Algorithm SHA256).Hash -ne $report.binaries[$name].sha256) {
            throw 'Installed binary changed during the run.'
        }
    }
    Assert-ChromeUnchanged
    $report.passed = $true
} catch {
    $report.failure = $_.Exception.Message
    throw
} finally {
    try {
        if ($restoreOwed) {
            $null = Invoke-Product @('native-host', 'install')
            foreach ($key in $registrations.Keys) {
                [IO.File]::WriteAllBytes($registrations[$key].path, $registrations[$key].bytes)
                Set-Item -LiteralPath $key -Value $registrations[$key].path
                if ((Get-Item -LiteralPath $key).GetValue('') -ne $registrations[$key].path -or
                    (Get-FileHash -LiteralPath $registrations[$key].path -Algorithm SHA256).Hash -ne $registrations[$key].hash) {
                    throw 'Original native-host state could not be restored.'
                }
            }
        }
    } catch {
        $report.passed = $false; $report.restoration_failure = $_.Exception.Message
        throw
    } finally {
        if ($null -ne $script:mcp) {
            if (-not $script:mcp.HasExited) { $script:mcp.Kill() }
            $script:mcp.Dispose()
        }
        $report.finished_at = [DateTime]::UtcNow.ToString('o'); Save-Evidence
        Write-Host ('Evidence: ' + (Join-Path $output 'results.json'))
    }
}
