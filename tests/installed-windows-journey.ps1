#Requires -Version 7.0
<#
.SYNOPSIS
Exercises the real installed Chrome/native-host/service path, then every MCP tool.
.DESCRIPTION
Opt-in, disruptive integration test for an idle development installation. Removes and restores
the owned native-host registration, stops the exact browser edge, and crashes the exact service
to prove that the MCP edge starts the next one and the browser reconnects through a new edge.
Chrome stays running. No browser settings or policy are changed.
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
$serviceImage = Join-Path $binRoot 'ghostlight-service.exe'
$edgeImage = Join-Path $binRoot 'ghostlight.exe'
foreach ($name in @('GHOSTLIGHT_TEST_HOME', 'GHOSTLIGHT_NATIVE_HOST_DIR', 'GHOSTLIGHT_PROFILE_DIR')) {
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
    $info.FileName = $edgeImage
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
# The browser edges: the edge image started by Chrome with an extension origin.
function Get-NativeEdge {
    @(Get-CimInstance Win32_Process -Filter "name='ghostlight.exe'" |
        Where-Object { $_.ExecutablePath -eq $edgeImage -and $_.CommandLine -match 'chrome-extension://' } |
        ForEach-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue } |
        Where-Object { $null -ne $_ })
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
function Invoke-Mcp([string]$Method, [hashtable]$Parameters) {
    $script:mcpRequest++
    $RequestId = $script:mcpRequest
    $frame = @{ jsonrpc = '2.0'; id = $RequestId; method = $Method; params = $Parameters }
    $script:mcp.StandardInput.WriteLine(($frame | ConvertTo-Json -Compress -Depth 10))
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
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
# The browsers the service has a connection to, as the session's list names them. A session that
# holds no tab answers its list without asking the browser, so the list is polled until the service
# names the browser it expects: exactly one browser at first, then that same persistent id after the
# service returns. A name proves the service registered that browser's connection.
function Wait-ListedBrowser([string]$Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    do {
        $listed = Invoke-Mcp 'tools/call' @{ name = 'browser_tabs'; arguments = @{ action = 'list' } }
        if ($listed.structuredContent.status -ne 'succeeded') { throw 'The initialized MCP stream could not list its tabs.' }
        $browsers = @($listed.structuredContent.facts.browsers | ForEach-Object { $_.browser })
        if ($browsers.Count -eq 1 -and (-not $Expected -or $browsers[0] -eq $Expected)) { return $browsers[0] }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The service did not name the connected browser within 45 seconds.'
}
$restoreOwed = $false
$registrations = @{}
$script:mcp = $null
$script:mcpRequest = 0
try {
    Save-Evidence
    foreach ($name in @('ghostlight.exe', 'ghostlight-service.exe')) {
        $path = Join-Path $binRoot $name
        $report.binaries[$name] = @{ path = $path; sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
    }
    $roots = @(Get-ChromeRoot)
    if ($roots.Count -ne 1) { throw 'Requires one running installed Chrome root.' }
    $script:chromeProcessId = $roots[0].ProcessId; $script:chromeCreated = $roots[0].CreationDate
    $report.chrome_pid = $script:chromeProcessId
    $report.chrome_version = (Get-Item -LiteralPath $chromeImage).VersionInfo.ProductVersion
    if ((Get-Doctor).readiness.state -ne 'ready') { throw 'Requires an idle, Ready installation.' }
    $authorities = @(Get-ExactProcess $serviceImage); $browserEdges = @(Get-NativeEdge)
    if ($authorities.Count -ne 1 -or $browserEdges.Count -ne 1) { throw 'Requires one exact installed service and browser edge.' }
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
        $path = [IO.Path]::GetFullPath($registryKey.GetValue(''))
        $profile = [IO.Path]::GetFullPath($env:USERPROFILE).TrimEnd([IO.Path]::DirectorySeparatorChar)
        $manifestFile = Get-Item -LiteralPath $path
        if (-not $path.StartsWith($profile + [IO.Path]::DirectorySeparatorChar,
                [StringComparison]::OrdinalIgnoreCase) -or
            $manifestFile.Name -ne 'org.sylin.ghostlight.json' -or
            $manifestFile.Directory.Name -ne 'manifests' -or
            $manifestFile.Directory.Parent.Name -ne '.ghostlight') {
            throw 'Requires a physical per-user Ghostlight manifest path.'
        }
        $manifest = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        if ($manifest.name -ne 'org.sylin.ghostlight' -or $manifest.path -ne $edgeImage) {
            throw 'Registration is foreign or belongs to another installation.'
        }
        $registrations[$key] = @{ path = $path; hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash;
            bytes = [IO.File]::ReadAllBytes($path) }
    }
    $restoreOwed = $true
    $null = Invoke-Product @('native-host', 'uninstall')
    foreach ($key in $keys) { if (Test-Path -LiteralPath $key) { throw 'Registration was not removed.' } }
    Stop-Exact $browserEdges[0] $edgeImage
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
    if ((Get-ExactProcess $serviceImage)[0].Id -ne $authority.Id) { throw 'The service restarted during registration recovery.' }
    Phase 'registration-reinstalled-with-Chrome-running' $timer.ElapsedMilliseconds

    $browserEdge = @(Get-NativeEdge)[0]
    Stop-Exact $browserEdge $edgeImage
    $timer.Restart()
    do {
        Start-Sleep -Milliseconds 250
        $replacement = @(Get-NativeEdge)
    } while (($replacement.Count -ne 1 -or $replacement[0].Id -eq $browserEdge.Id) -and $timer.Elapsed.TotalSeconds -lt 45)
    if ($replacement.Count -ne 1 -or $replacement[0].Id -eq $browserEdge.Id) { throw 'Chrome did not replace its crashed browser edge.' }
    Wait-Readiness 'ready'
    Phase 'browser-edge-crash-recovers-automatically' $timer.ElapsedMilliseconds

    $browserEdge = $replacement[0]
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $edgeImage
    $info.ArgumentList.Add('mcp')
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true; $info.RedirectStandardOutput = $true
    $script:mcp = [Diagnostics.Process]::new(); $script:mcp.StartInfo = $info
    $null = $script:mcp.Start()
    $initialized = Invoke-Mcp 'initialize' @{ protocolVersion = '2025-11-25'; capabilities = @{};
        clientInfo = @{ name = 'Installed recovery acceptance'; version = '1' } }
    if ($initialized.serverInfo.name -ne 'ghostlight') { throw 'Unexpected MCP server identity.' }
    $script:mcp.StandardInput.WriteLine('{"jsonrpc":"2.0","method":"notifications/initialized"}')
    $connected = Wait-ListedBrowser ''
    $report.retained_mcp_pid = $script:mcp.Id
    Stop-Exact $authority $serviceImage
    $timer.Restart()
    # The MCP edge keeps its stream and starts the next service when a request needs it. The browser
    # edge exits once that service answers, and Chrome starts a new one.
    $relisted = Invoke-Mcp 'tools/list' @{}
    if (@($relisted.tools).Count -eq 0 -or $script:mcp.HasExited) { throw 'The initialized MCP stream did not survive the lost service.' }
    Wait-Readiness 'ready'
    $restarted = @(Get-ExactProcess $serviceImage)
    if ($restarted.Count -ne 1 -or $restarted[0].Id -eq $authority.Id) { throw 'A new service did not answer.' }
    do {
        Start-Sleep -Milliseconds 250
        $reconnected = @(Get-NativeEdge)
    } while (($reconnected.Count -ne 1 -or $reconnected[0].Id -eq $browserEdge.Id) -and $timer.Elapsed.TotalSeconds -lt 45)
    if ($reconnected.Count -ne 1 -or $reconnected[0].Id -eq $browserEdge.Id) { throw 'Chrome did not start a new browser edge after the service returned.' }
    $null = Wait-ListedBrowser $connected
    if ($script:mcp.HasExited) { throw 'The initialized MCP stream did not survive the recovery.' }
    Phase 'service-crash-keeps-the-MCP-stream-and-the-browser-reconnects' $timer.ElapsedMilliseconds
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
