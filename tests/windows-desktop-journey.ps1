# Exercise the real Windows workbench lifetime, including Open racing the service's startup.
# The journey runs in a test home under .tmp holding its own copies of the edge, the service and
# the shell. Every window and process operation addresses only exact images of that home.
param(
    [string] $BinDir = $env:GHOSTLIGHT_BIN_DIR,
    [ValidateRange(1, 10)] [int] $StartupRounds = 3,
    [ValidateRange(2, 24)] [int] $OpenBurst = 8
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This journey requires an interactive Windows desktop.' }
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $BinDir) { $BinDir = Join-Path $repository '.target-ghostlight-1.0/debug' }
$stamp = [Guid]::NewGuid().ToString('N')
$evidence = Join-Path $repository ".tmp/native-desktop-$stamp"
$ghostlightHome = Join-Path $evidence 'home'
$null = New-Item -ItemType Directory -Path (Join-Path $ghostlightHome 'bin')
foreach ($name in @('ghostlight.exe', 'ghostlight-service.exe', 'ghostlight-desktop.exe')) {
    $source = Join-Path $BinDir $name
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing $source; build the workspace first." }
    Copy-Item -LiteralPath $source -Destination (Join-Path $ghostlightHome 'bin')
}
$edgeImage = (Resolve-Path -LiteralPath (Join-Path $ghostlightHome 'bin/ghostlight.exe')).Path
$serviceImage = (Resolve-Path -LiteralPath (Join-Path $ghostlightHome 'bin/ghostlight-service.exe')).Path
$shellImage = (Resolve-Path -LiteralPath (Join-Path $ghostlightHome 'bin/ghostlight-desktop.exe')).Path
$env:GHOSTLIGHT_TEST_HOME = $ghostlightHome
$env:GHOSTLIGHT_AUDIT_FILE = Join-Path $evidence 'audit.jsonl'
$env:GHOSTLIGHT_POLICY_FILE = Join-Path $evidence 'policy.json'
@{
    schema=4; name='Native desktop fixture'; version='1'
    grants=@(@{ id='ordinary'; hosts=@{ allow=@('*') }; allowed=@('read', 'action', 'write', 'execute') })
    config=@(@{ key='browser.startup'; value='manual'; level='mandatory' })
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $env:GHOSTLIGHT_POLICY_FILE -Encoding utf8
$env:GHOSTLIGHT_NATIVE_HOST_DIR = Join-Path $evidence 'native-host'
$env:GHOSTLIGHT_DIAGNOSTICS_DIR = Join-Path $evidence 'diagnostics'
$hashes = @{}
foreach ($image in @($edgeImage, $serviceImage, $shellImage)) {
    $hashes[[IO.Path]::GetFileName($image)] = (Get-FileHash -LiteralPath $image -Algorithm SHA256).Hash
}
Write-Output "Native desktop images: $($hashes.Keys -join ', ') from $BinDir"
Write-Output "Native desktop evidence: $evidence"

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class GhostlightNativeJourney {
    public class Window {
        public long Handle;
        public int Owner;
        public bool Visible;
        public bool Minimized;
        public bool Responsive;
    }
    private delegate bool EnumProc(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int limit);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] private static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr w, IntPtr l, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll")] private static extern bool ShowWindowAsync(IntPtr window, int command);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window, uint message, UIntPtr w, IntPtr l);
    public static Window[] Inspect(int owner) {
        var result = new List<Window>();
        EnumWindows((window, data) => {
            uint process;
            GetWindowThreadProcessId(window, out process);
            if (process != owner) return true;
            var name = new StringBuilder(256);
            GetClassName(window, name, name.Capacity);
            if (name.ToString() != "Ghostlight Window") return true;
            UIntPtr ignored;
            result.Add(new Window {
                Handle=window.ToInt64(), Owner=owner, Visible=IsWindowVisible(window), Minimized=IsIconic(window),
                Responsive=SendMessageTimeout(window, 0, UIntPtr.Zero, IntPtr.Zero, 2, 250, out ignored) != IntPtr.Zero
            });
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
    public static void Control(int owner, long handle, bool close) {
        var window = new IntPtr(handle);
        uint process;
        GetWindowThreadProcessId(window, out process);
        if (process != owner) throw new InvalidOperationException("Window ownership changed.");
        if (close) {
            if (!PostMessage(window, 0x0010, UIntPtr.Zero, IntPtr.Zero)) throw new InvalidOperationException("WM_CLOSE failed.");
        } else if (!ShowWindowAsync(window, 6)) throw new InvalidOperationException("Minimize failed.");
    }
    // Every Ghostlight process owns one session listener window (class GhostlightSessionListener_*).
    // This sends it the end-session messages Windows sends at sign-out and shutdown.
    public static void EndSession(int owner) {
        IntPtr listener = IntPtr.Zero;
        EnumWindows((window, data) => {
            uint process;
            GetWindowThreadProcessId(window, out process);
            if (process != owner) return true;
            var name = new StringBuilder(256);
            GetClassName(window, name, name.Capacity);
            if (name.ToString().StartsWith("GhostlightSessionListener_")) listener = window;
            return true;
        }, IntPtr.Zero);
        if (listener == IntPtr.Zero) throw new InvalidOperationException("Session listener window is unavailable.");

        UIntPtr result;
        if (SendMessageTimeout(listener, 0x0011, UIntPtr.Zero, IntPtr.Zero, 2, 1000, out result) == IntPtr.Zero || result.ToUInt64() != 1)
            throw new InvalidOperationException("WM_QUERYENDSESSION was not accepted promptly.");
        // The process may exit before this send returns, so only the earlier query has a reply
        // assertion.
        SendMessageTimeout(listener, 0x0016, new UIntPtr(1), IntPtr.Zero, 2, 250, out result);
    }
}
'@

$children = [Collections.Generic.List[Diagnostics.Process]]::new()
$edgeChildren = [Collections.Generic.List[Diagnostics.Process]]::new()
$observations = [Collections.Generic.List[object]]::new()
$script:launch = 0
# The service starts with the serve verb. Starters that lose the race for the service lock stay
# alive for the lock wait of the service (LEAVING_LOCK_WAIT in service/lifetime.rs).
$servePattern = '\sserve(\s|$)'
$LeavingLockWaitSeconds = 5
$LeavingLockMarginSeconds = 10
$failure = $null

function Start-OwnedProcess([string] $FilePath, [string[]] $Arguments = @()) {
    $script:launch++
    $options = @{
        FilePath = $FilePath; PassThru = $true; WindowStyle = 'Hidden'
        RedirectStandardOutput = (Join-Path $evidence "$script:launch.stdout.log")
        RedirectStandardError = (Join-Path $evidence "$script:launch.stderr.log")
    }
    if ($Arguments.Count) { $options.ArgumentList = $Arguments }
    $child = Start-Process @options
    $children.Add($child)
    return $child
}

# The processes whose executable is exactly `$Image`, an image of this journey's own home.
function Get-ImageProcesses([string] $Image) {
    @(Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Image)) -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -ieq $Image })
}

# The services of this home are the processes of its service image started with the serve verb.
# The service image also runs short callers such as open, which are not services.
function Get-ServeProcesses {
    $name = [IO.Path]::GetFileName($serviceImage)
    @(Get-CimInstance -ClassName Win32_Process -Filter "Name = '$name'" |
        Where-Object { $_.ExecutablePath -ieq $serviceImage -and $_.CommandLine -match $servePattern })
}

# The one service of this home. Every caller that finds none starts its own serve process and the
# starters that lose the race wait out the lock wait before they leave, so more than one serve
# process is expected until they have. Call it once the callers have exited, so no further starter
# can appear, and it waits for exactly one serve process to remain.
function Wait-SingleService {
    $deadline = [DateTime]::UtcNow.AddSeconds($LeavingLockWaitSeconds + $LeavingLockMarginSeconds)
    do {
        $serving = @(Get-ServeProcesses)
        if ($serving.Count -eq 1) { return (Get-Process -Id $serving[0].ProcessId) }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Expected one service of this home after the starters left, found $(@(Get-ServeProcesses).Count)."
}

function Get-OwnedService {
    $services = @(Get-ServeProcesses)
    if ($services.Count -ne 1) { throw "Expected one service of this home, found $($services.Count)." }
    return (Get-Process -Id $services[0].ProcessId)
}

# The window belongs to the shell. A second shell exits at once, so at most one window exists in
# any process of this home at any time, however many starters are still racing for the service.
function Get-OwnedWindows {
    $windows = @()
    foreach ($shell in @(Get-ImageProcesses $shellImage)) {
        $windows += @([GhostlightNativeJourney]::Inspect($shell.Id))
    }
    if ($windows.Count -gt 1) {
        $details = $windows | ConvertTo-Json -Compress
        throw "Duplicate native workbenches in this home: $details"
    }
    return $windows
}

function Wait-ServiceRunning {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $text = & $edgeImage status --json 2>$null
        if ($LASTEXITCODE -eq 0 -and $text) {
            $status = $text | ConvertFrom-Json
            if ($status.running) { return $status }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The service did not answer status.'
}

function Wait-WindowState([string] $State) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $windows = @(Get-OwnedWindows)
        $ready = switch ($State) {
            'closed' { $windows.Count -eq 0 }
            'minimized' { $windows.Count -eq 1 -and $windows[0].Responsive -and $windows[0].Minimized }
            'restored' { $windows.Count -eq 1 -and $windows[0].Responsive -and $windows[0].Visible -and -not $windows[0].Minimized }
            default { throw "Unknown native state $State" }
        }
        if ($ready) { return $windows }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $State workbench."
}

function Wait-OpenBurst([object[]] $Openers) {
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    do {
        $pending = 0
        foreach ($opener in $Openers) {
            $opener.Refresh()
            if (-not $opener.HasExited) { $pending++; continue }
            if ($opener.ExitCode -ne 0) { throw "Open process $($opener.Id) failed: $($opener.ExitCode)." }
        }
        # Windows are counted on every pass: the first openers race to start the service.
        $null = @(Get-OwnedWindows)
        if ($pending -eq 0) { return }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Concurrent Open requests did not settle.'
}

function Record-State([int] $Round, [string] $State) {
    $windows = @(Get-OwnedWindows)
    $observations.Add(@{ round=$Round; state=$State; pid=(Get-OwnedService).Id; windows=$windows })
    Write-Output "PASS round=$Round $State count=$($windows.Count)"
}

function Assert-IsolatedWebView([int] $Round, [int] $ShellProcessId) {
    # Query only children of the exact shell of this home. Never collect unrelated browser profiles.
    # The parameter is not named ShellId: that is a read-only automatic variable of PowerShell.
    $webViews = @(Get-CimInstance -ClassName Win32_Process -Filter "ParentProcessId = $ShellProcessId AND Name = 'msedgewebview2.exe'")
    $profiles = @()
    foreach ($webView in $webViews) {
        if ($webView.CommandLine -match '(?:^|\s)--user-data-dir=(?:"([^"]+)"|(\S+))') {
            $directory = if ($Matches[1]) { $Matches[1] } else { $Matches[2] }
            $profiles += [IO.Path]::GetFullPath($directory)
        }
    }
    # The window keeps one persistent profile under the home, state/desktop-webview (ADR-0199
    # Decision 19, as amended by "shell milestone 2"; owner answer Q7). WebView2 keeps Chromium's browser profile in EBWebView beneath it. Every
    # WebView2 process of the shell must use exactly that folder, so the window shares a profile
    # with neither Edge nor any other application.
    $expected = [IO.Path]::GetFullPath((Join-Path $ghostlightHome 'state/desktop-webview/EBWebView'))
    if (-not $profiles.Count -or @($profiles | Where-Object { $_ -ine $expected }).Count) {
        throw "The shell WebView2 did not use the profile directory of this home: expected $expected, found $($profiles -join ', ')."
    }
    $observations.Add(@{ round=$Round; state='isolated-webview-profile'; pid=$ShellProcessId; directory=$expected })
    Write-Output "PASS round=$Round isolated-webview-profile"
}

function Stop-ImageProcesses([string] $Image) {
    foreach ($process in @(Get-ImageProcesses $Image)) {
        Stop-Process -InputObject $process -Force
        if (-not $process.WaitForExit(5000)) { throw "Owned process $($process.Id) did not exit." }
    }
}

function Stop-OwnedProcess([Diagnostics.Process] $Child) {
    $Child.Refresh()
    if (-not $Child.HasExited) {
        Stop-Process -InputObject $Child -Force
        if (-not $Child.WaitForExit(5000)) { throw "Owned child $($Child.Id) did not exit." }
    }
}

# Starts the edge with held input and a verb, waits for its session listener, delivers the
# end-session sequence and requires a prompt, successful exit.
function Assert-EdgeShutdown([string[]] $Arguments, [string] $Label) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $edgeImage
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $edge = [Diagnostics.Process]::new()
    $edge.StartInfo = $info
    if (-not $edge.Start()) { throw "Could not start the edge for $Label." }
    $edgeChildren.Add($edge)

    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $listenerReady = $false
    do {
        $edge.Refresh()
        if ($edge.HasExited) { throw "The $Label edge exited before its session listener was ready." }
        try {
            [GhostlightNativeJourney]::EndSession($edge.Id)
            $listenerReady = $true
            break
        } catch {
            if ($_.Exception.Message -notmatch 'Session listener window is unavailable') { throw }
        }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $listenerReady) { throw "The $Label edge did not create its session listener." }
    if (-not $edge.WaitForExit(4000)) { throw "The $Label edge ignored WM_ENDSESSION." }
    if ($edge.ExitCode -ne 0) { throw "The $Label edge ended session with status $($edge.ExitCode)." }
    $observations.Add(@{ round=0; state='edge-shutdown-exit'; process=$Label; pid=$edge.Id; windows=@() })
    Write-Output "PASS process=$Label system-shutdown-exit"
}

try {
    Assert-EdgeShutdown @('chrome-extension://abcdefghijklmnopabcdefghijklmnop/') 'browser'
    # A browser relay starts the service only after the extension's hello.
    if (@(Get-ImageProcesses $serviceImage).Count) { throw 'A browser edge with no hello started a service.' }
    Assert-EdgeShutdown @('mcp') 'mcp'
    # The MCP relay starts the service at its own start, an agent start (ADR-0199 Decision 10).
    $null = Wait-ServiceRunning
    $agentService = Wait-SingleService
    # End that session so round 1 races a cold start; the service says goodbye to any shell.
    [GhostlightNativeJourney]::EndSession($agentService.Id)
    if (-not $agentService.WaitForExit(6000)) { throw "Service $($agentService.Id) ignored WM_ENDSESSION." }
    $deadline = [DateTime]::UtcNow.AddSeconds(6)
    while ((@(Get-ImageProcesses $shellImage).Count -or @(Get-ImageProcesses $serviceImage).Count) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
    if (@(Get-ImageProcesses $serviceImage).Count -or @(Get-ImageProcesses $shellImage).Count) { throw 'The agent-started service or its shell outlived the end of the session.' }
    $observations.Add(@{ round=0; state='mcp-edge-agent-start'; pid=$agentService.Id; windows=@() })
    Write-Output 'PASS process=mcp agent-start-at-own-start'

    for ($round = 1; $round -le $StartupRounds; $round++) {
        # A person's start: the openers race to start the service, one wins, and the shell brings up
        # one window. Open requests keep arriving while the service starts.
        $openers = @(for ($index = 0; $index -lt $OpenBurst; $index++) { Start-OwnedProcess $serviceImage @('open') })
        Wait-OpenBurst $openers
        $null = Wait-ServiceRunning
        $service = Wait-SingleService
        $window = @(Wait-WindowState 'restored')[0]
        for ($sample = 0; $sample -lt 10; $sample++) {
            $null = @(Get-OwnedWindows)
            Start-Sleep -Milliseconds 50
        }
        Record-State $round 'startup-open-burst'
        Assert-IsolatedWebView $round $window.Owner

        $minimizedHandle = $window.Handle
        [GhostlightNativeJourney]::Control($window.Owner, $window.Handle, $false)
        $null = @(Wait-WindowState 'minimized')
        Record-State $round 'native-minimize'
        Wait-OpenBurst @((Start-OwnedProcess $serviceImage @('open')))
        $window = @(Wait-WindowState 'restored')[0]
        if ($window.Handle -ne $minimizedHandle) { throw 'Open replaced the minimized native window instead of restoring it.' }
        Record-State $round 'restore-existing'

        [GhostlightNativeJourney]::Control($window.Owner, $window.Handle, $true)
        $null = @(Wait-WindowState 'closed')
        if ((Get-OwnedService).Id -ne $service.Id) { throw 'The service ended when its window closed.' }
        Record-State $round 'native-close-service-alive'
        $openers = @(for ($index = 0; $index -lt $OpenBurst; $index++) { Start-OwnedProcess $serviceImage @('open') })
        Wait-OpenBurst $openers
        $null = @(Wait-WindowState 'restored')
        Record-State $round 'reopen-burst-one-replacement'

        # The end of the session reaches the service, which says goodbye to the shell; both leave.
        [GhostlightNativeJourney]::EndSession($service.Id)
        if (-not $service.WaitForExit(6000)) { throw "Service $($service.Id) ignored WM_ENDSESSION." }
        $deadline = [DateTime]::UtcNow.AddSeconds(6)
        while (@(Get-ImageProcesses $shellImage).Count -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
        if (@(Get-ImageProcesses $shellImage).Count) { throw 'The shell outlived the end of the session.' }
        $observations.Add(@{ round=$round; state='system-shutdown-exit'; pid=$service.Id; windows=@() })
        Write-Output "PASS round=$round system-shutdown-exit"
    }
    Write-Output "Windows native desktop journey passed: $($observations.Count) lifecycle checks."
} catch {
    $failure = $_.Exception.Message
    throw
} finally {
    $cleanupFailures = @()
    # Stop Open callers first so none can start a replacement during cleanup.
    for ($index = $children.Count - 1; $index -ge 0; $index--) {
        try { Stop-OwnedProcess $children[$index] }
        catch { $cleanupFailures += $_.Exception.Message }
    }
    foreach ($edge in $edgeChildren) {
        try { Stop-OwnedProcess $edge } catch { $cleanupFailures += $_.Exception.Message }
    }
    foreach ($image in @($shellImage, $serviceImage)) {
        try { Stop-ImageProcesses $image } catch { $cleanupFailures += $_.Exception.Message }
    }
    $failureCode = if ($failure) { 'native_lifecycle' } elseif ($cleanupFailures.Count) { 'cleanup' } else { $null }
    @{ images=$hashes; passed=($null -eq $failureCode); failure_code=$failureCode; failure=$failure; cleanup_failures=$cleanupFailures; rounds=$StartupRounds; open_burst=$OpenBurst; observations=$observations } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'results.json') -Encoding utf8
    if (-not $cleanupFailures.Count) {
        # The images are the only bulk; the evidence stays.
        try { Remove-Item -LiteralPath (Join-Path $ghostlightHome 'bin') -Recurse -Force } catch { Write-Warning "Could not remove the copied images: $($_.Exception.Message)" }
    }
    if ($cleanupFailures.Count -and -not $failure) { throw ($cleanupFailures -join '; ') }
}
